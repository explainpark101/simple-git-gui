//! AI-assisted commit message generation (Changes view).
//!
//! Uses the multi-provider registry in Settings. Auto tries enabled providers
//! in list order. Never commits — fills the commit textarea for review.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sgg_core::{prepare_commit_context, CommitMessageContext};

use super::ai_provider::{AiAuthKind, AiProvider, AiProviderKind};
use super::prefs::{CommitAiPrefs, CommitAiStyle};

/// Connect timeout stays short even when read/wait timeout is unlimited.
const CONNECT_TIMEOUT_SECS: u64 = 15;

pub type AbortFlag = Arc<AtomicBool>;

pub fn check_aborted(abort: Option<&AbortFlag>) -> Result<()> {
    if abort.is_some_and(|a| a.load(Ordering::Relaxed)) {
        bail!("aborted");
    }
    Ok(())
}

pub fn is_abort_error(err: &anyhow::Error) -> bool {
    format!("{err:#}").contains("aborted")
}

#[derive(Debug, Clone)]
pub struct GeneratedCommitMessage {
    pub subject: String,
    pub body: String,
}

impl GeneratedCommitMessage {
    pub fn combined(&self) -> String {
        if self.body.trim().is_empty() {
            self.subject.clone()
        } else {
            format!("{}\n\n{}", self.subject.trim(), self.body.trim())
        }
    }
}

/// Build git context + call the configured provider(s).
///
/// `on_partial` receives the accumulated assistant text during HTTP streaming.
pub fn generate_for_repo(
    repo: &Path,
    user_hint: Option<&str>,
    prefs: &CommitAiPrefs,
    abort: Option<&AbortFlag>,
    mut on_partial: impl FnMut(&str),
) -> Result<GeneratedCommitMessage> {
    check_aborted(abort)?;
    let ctx = prepare_commit_context(repo, user_hint).context("prepare commit context")?;
    if ctx.is_empty() {
        bail!("no staged, unstaged, or untracked changes to describe");
    }
    generate_from_context(repo, &ctx, prefs, abort, &mut on_partial)
}

fn generate_from_context(
    repo: &Path,
    ctx: &CommitMessageContext,
    prefs: &CommitAiPrefs,
    abort: Option<&AbortFlag>,
    on_partial: &mut dyn FnMut(&str),
) -> Result<GeneratedCommitMessage> {
    let prompt = build_prompt(ctx, prefs.style);
    if prefs.is_auto() {
        let mut errors = Vec::new();
        let mut tried = 0usize;
        for provider in prefs.providers.iter().filter(|p| p.enabled) {
            check_aborted(abort)?;
            if !provider_usable(provider, prefs) {
                continue;
            }
            tried += 1;
            match generate_with_provider(repo, &prompt, provider, prefs, abort, on_partial) {
                Ok(msg) => return Ok(msg),
                Err(err) if is_abort_error(&err) => return Err(err),
                Err(err) => errors.push(format!("{}: {err:#}", provider.title)),
            }
        }
        if tried == 0 {
            bail!("no ready AI providers — enable one and set API key / CLI in Settings");
        }
        bail!(
            "no AI provider succeeded:\n{}",
            errors
                .into_iter()
                .map(|e| format!("- {e}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    let id = prefs.active_provider_id.trim();
    let provider = prefs
        .provider_by_id(id)
        .ok_or_else(|| anyhow!("provider `{id}` not found"))?;
    if !provider.enabled {
        bail!("provider `{}` is disabled", provider.title);
    }
    generate_with_provider(repo, &prompt, provider, prefs, abort, on_partial)
}

fn provider_usable(provider: &AiProvider, prefs: &CommitAiPrefs) -> bool {
    match provider.kind {
        AiProviderKind::OpenAiCompatible => provider.openai_ready(),
        AiProviderKind::CursorCli => resolve_cursor_bin(provider, prefs).is_ok(),
        AiProviderKind::CodexCli => resolve_codex_bin(provider, prefs).is_ok(),
    }
}

fn generate_with_provider(
    repo: &Path,
    prompt: &str,
    provider: &AiProvider,
    prefs: &CommitAiPrefs,
    abort: Option<&AbortFlag>,
    on_partial: &mut dyn FnMut(&str),
) -> Result<GeneratedCommitMessage> {
    match provider.kind {
        AiProviderKind::OpenAiCompatible => {
            generate_http(prompt, provider, prefs, abort, on_partial)
        }
        AiProviderKind::CursorCli => {
            let bin = resolve_cursor_bin(provider, prefs).or_else(|_| {
                super::cli_tools::ensure_cursor().context("install Cursor Agent CLI")
            })?;
            generate_cursor_with_bin(repo, prompt, provider, &bin, prefs, abort)
        }
        AiProviderKind::CodexCli => {
            let bin = resolve_codex_bin(provider, prefs).or_else(|_| {
                super::cli_tools::ensure_codex().context("install Codex CLI")
            })?;
            generate_codex_with_bin(repo, prompt, provider, &bin, prefs, abort)
        }
    }
}

fn build_prompt(ctx: &CommitMessageContext, style: CommitAiStyle) -> String {
    let style_rules = match style {
        CommitAiStyle::Conventional => {
            "Use Conventional Commits for subject: type(optional-scope): subject.\n\
             Types: feat, fix, docs, style, refactor, test, chore.\n\
             Subject: imperative mood, ~50 chars, no trailing period."
        }
        CommitAiStyle::Free => {
            "Subject: clear imperative mood, ~50–72 chars, no trailing period."
        }
    };

    let mut out = String::new();
    // Match T3 commit prompt shape: structured JSON {subject,body}.
    out.push_str(
        "You write concise git commit messages.\n\
         Return a JSON object with keys: subject, body.\n\
         Rules:\n\
         - subject must be imperative, <= 72 chars, and no trailing period\n\
         - body can be empty string or short bullet points\n\
         - capture the primary user-visible or developer-visible change\n\
         - do not invent files not in the diff; never paste secrets\n",
    );
    out.push_str(style_rules);
    out.push_str("\n\n");
    if let Some(branch) = ctx.branch.as_deref() {
        out.push_str(&format!("Branch: {branch}\n"));
    }
    out.push_str(if ctx.used_staged {
        "Source: staged changes (index).\n"
    } else {
        "Source: working tree (nothing staged; describe unstaged/untracked).\n"
    });
    if let Some(hint) = ctx.user_hint.as_deref() {
        out.push_str(&format!("User hint (prefer this intent): {hint}\n"));
    }
    if !ctx.recent_subjects.is_empty() {
        out.push_str("\nRecent commit subjects (match tone):\n");
        for s in &ctx.recent_subjects {
            out.push_str("- ");
            out.push_str(s);
            out.push('\n');
        }
    }
    out.push_str("\nChanged files:\n");
    out.push_str(&ctx.summary);
    out.push_str("\n\nDiff:\n");
    out.push_str(&ctx.patch);
    out.push('\n');
    out
}

#[derive(Deserialize)]
struct CommitJson {
    #[serde(default)]
    subject: String,
    #[serde(default)]
    body: String,
}

fn parse_message(raw: &str) -> GeneratedCommitMessage {
    let text = strip_fences(raw).trim().to_string();
    if let Ok(parsed) = serde_json::from_str::<CommitJson>(&text) {
        let subject = parsed.subject.trim().trim_matches('"').to_string();
        if !subject.is_empty() {
            return GeneratedCommitMessage {
                subject,
                body: parsed.body.trim().to_string(),
            };
        }
    }
    // Tolerate models that wrap JSON in prose.
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                let slice = &text[start..=end];
                if let Ok(parsed) = serde_json::from_str::<CommitJson>(slice) {
                    let subject = parsed.subject.trim().trim_matches('"').to_string();
                    if !subject.is_empty() {
                        return GeneratedCommitMessage {
                            subject,
                            body: parsed.body.trim().to_string(),
                        };
                    }
                }
            }
        }
    }
    let mut lines = text.lines();
    let subject = lines
        .next()
        .unwrap_or("chore: update")
        .trim()
        .trim_matches('"')
        .to_string();
    let rest: Vec<&str> = lines.collect();
    let body = rest.join("\n").trim().to_string();
    GeneratedCommitMessage { subject, body }
}

fn strip_fences(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        let rest = rest
            .strip_prefix("text")
            .or_else(|| rest.strip_prefix("markdown"))
            .or_else(|| rest.strip_prefix("commit"))
            .unwrap_or(rest);
        let rest = rest.trim_start_matches('\n');
        if let Some(end) = rest.rfind("```") {
            return rest[..end].trim().to_string();
        }
        return rest.trim().to_string();
    }
    trimmed.to_string()
}

// --- HTTP (OpenAI-compatible) -------------------------------------------------

fn http_agent(prefs: &CommitAiPrefs) -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .user_agent(&format!("simple-git-gui/{}", env!("CARGO_PKG_VERSION")));
    // Empty / None timeout → do not enforce a read deadline (week-long stand-in).
    if let Some(secs) = prefs.timeout_secs.filter(|s| *s > 0) {
        builder = builder.timeout_read(Duration::from_secs(secs));
    } else {
        builder = builder.timeout_read(Duration::from_secs(60 * 60 * 24 * 7));
    }
    builder.build()
}

fn generate_http(
    prompt: &str,
    provider: &AiProvider,
    prefs: &CommitAiPrefs,
    abort: Option<&AbortFlag>,
    on_partial: &mut dyn FnMut(&str),
) -> Result<GeneratedCommitMessage> {
    check_aborted(abort)?;
    if provider.base_url.trim().is_empty() {
        bail!("base URL is empty");
    }
    if provider.model.trim().is_empty() {
        bail!("model is empty");
    }
    let mut url = provider.chat_completions_url();
    let key = provider.resolve_api_key();
    if provider.auth == AiAuthKind::Query {
        let param = if provider.auth_query_param.trim().is_empty() {
            "key"
        } else {
            provider.auth_query_param.trim()
        };
        let Some(key) = key.as_deref() else {
            bail!("API key required for query auth");
        };
        let sep = if url.contains('?') { '&' } else { '?' };
        url = format!("{url}{sep}{param}={}", urlencoding_minimal(key));
    }

    let stream = prefs.http_mode.is_streaming();
    let body = ChatRequest {
        model: provider.model.trim().to_string(),
        temperature: 0.2,
        stream,
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: "You are a git commit message assistant.".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: prompt.to_string(),
            },
        ],
    };
    let agent = http_agent(prefs);
    let mut req = agent.post(&url).set("Content-Type", "application/json");
    if stream {
        req = req.set("Accept", "text/event-stream");
    }
    match provider.auth {
        AiAuthKind::None | AiAuthKind::Query => {}
        AiAuthKind::Bearer => {
            let Some(key) = key.as_deref() else {
                bail!("API key required for Bearer auth");
            };
            req = req.set("Authorization", &format!("Bearer {key}"));
        }
        AiAuthKind::Header => {
            let Some(key) = key.as_deref() else {
                bail!("API key required for header auth");
            };
            let header = if provider.auth_header.trim().is_empty() {
                "x-api-key"
            } else {
                provider.auth_header.trim()
            };
            req = req.set(header, key);
        }
    }
    check_aborted(abort)?;
    let response = req
        .send_json(&body)
        .with_context(|| format!("POST {url}"))?;
    if stream {
        read_chat_sse(response, abort, on_partial)
    } else {
        let parsed: ChatResponse = response.into_json().context("parse chat completions JSON")?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message)
            .map(|m| m.content)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| anyhow!("empty model response"))?;
        on_partial(&content);
        Ok(parse_message(&content))
    }
}

fn read_chat_sse(
    response: ureq::Response,
    abort: Option<&AbortFlag>,
    on_partial: &mut dyn FnMut(&str),
) -> Result<GeneratedCommitMessage> {
    let reader = response.into_reader();
    let mut lines = BufReader::new(reader);
    let mut line = String::new();
    let mut accumulated = String::new();
    loop {
        check_aborted(abort)?;
        line.clear();
        let n = lines.read_line(&mut line).context("read SSE line")?;
        if n == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        let Some(data) = trimmed.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() {
            continue;
        }
        if data == "[DONE]" {
            break;
        }
        let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) else {
            continue;
        };
        for choice in chunk.choices {
            if let Some(delta) = choice.delta.and_then(|d| d.content) {
                if delta.is_empty() {
                    continue;
                }
                accumulated.push_str(&delta);
                on_partial(&accumulated);
            }
        }
    }
    check_aborted(abort)?;
    if accumulated.trim().is_empty() {
        bail!("empty streamed model response");
    }
    Ok(parse_message(&accumulated))
}

/// Minimal query escaping (keys rarely need more).
fn urlencoding_minimal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    temperature: f32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    #[serde(default)]
    message: Option<ChatMessageOut>,
}

#[derive(Deserialize)]
struct ChatMessageOut {
    #[serde(default)]
    content: String,
}

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: Option<StreamDelta>,
}

#[derive(Deserialize)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
}

// --- Cursor / Codex (T3-aligned commit text generation) -----------------------

const COMMIT_OUTPUT_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "subject": { "type": "string" },
    "body": { "type": "string" }
  },
  "required": ["subject", "body"],
  "additionalProperties": false
}"#;

fn generate_cursor_with_bin(
    repo: &Path,
    prompt: &str,
    provider: &AiProvider,
    bin: &str,
    prefs: &CommitAiPrefs,
    abort: Option<&AbortFlag>,
) -> Result<GeneratedCommitMessage> {
    check_aborted(abort)?;
    // T3 V2 uses @cursor/sdk in-process; Rust can't load that. Auth matches T3
    // (CURSOR_API_KEY / provider key). Invocation uses Cursor's documented
    // headless CLI: `agent -p` with the key in the environment.
    let workspace = repo.to_string_lossy();
    let mut args = vec![
        "-p".to_string(),
        "--mode".into(),
        "ask".into(),
        "--trust".into(),
        "--workspace".into(),
        workspace.as_ref().to_string(),
        "--output-format".into(),
        "text".into(),
    ];
    if !provider.model.trim().is_empty() {
        args.push("--model".into());
        args.push(provider.model.trim().to_string());
    }
    args.push(prompt.to_string());
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let mut env: Vec<(String, String)> = Vec::new();
    if let Some(key) = provider.resolve_api_key() {
        env.push(("CURSOR_API_KEY".into(), key));
    }
    let env_refs: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    let output = run_command_capture(
        bin,
        &arg_refs,
        Some(repo),
        None,
        &env_refs,
        prefs.timeout_secs,
        abort,
    )?;
    if output.trim().is_empty() {
        bail!("cursor agent returned empty output (sign in or set CURSOR_API_KEY)");
    }
    Ok(parse_message(&output))
}

fn generate_codex_with_bin(
    repo: &Path,
    prompt: &str,
    provider: &AiProvider,
    bin: &str,
    prefs: &CommitAiPrefs,
    abort: Option<&AbortFlag>,
) -> Result<GeneratedCommitMessage> {
    check_aborted(abort)?;
    // Same one-shot path as T3 CodexTextGeneration: exec + schema + last-message file.
    let tmp = std::env::temp_dir().join(format!(
        "sgg-codex-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&tmp).with_context(|| format!("create {}", tmp.display()))?;
    let schema_path = tmp.join("schema.json");
    let output_path = tmp.join("out.json");
    fs::write(&schema_path, COMMIT_OUTPUT_SCHEMA)
        .with_context(|| format!("write {}", schema_path.display()))?;
    fs::write(&output_path, "").with_context(|| format!("write {}", output_path.display()))?;

    let schema_s = schema_path.to_string_lossy().into_owned();
    let output_s = output_path.to_string_lossy().into_owned();
    let mut args = vec![
        "exec".to_string(),
        "--config".into(),
        "features.plugins=false".into(),
        "--config".into(),
        "features.apps=false".into(),
        "--ephemeral".into(),
        "--skip-git-repo-check".into(),
        "-s".into(),
        "read-only".into(),
        "--config".into(),
        "features.codex_hooks=false".into(),
        "--config".into(),
        "features.hooks=false".into(),
        "--output-schema".into(),
        schema_s,
        "--output-last-message".into(),
        output_s,
    ];
    if !provider.model.trim().is_empty() {
        args.push("--model".into());
        args.push(provider.model.trim().to_string());
        args.push("--config".into());
        args.push("model_reasoning_effort=\"medium\"".into());
    }
    args.push("-".into());
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let result = run_command_capture(
        bin,
        &arg_refs,
        Some(repo),
        Some(prompt),
        &[],
        prefs.timeout_secs,
        abort,
    );
    let file_raw = fs::read_to_string(&output_path).unwrap_or_default();
    let _ = fs::remove_dir_all(&tmp);

    match result {
        Ok(_) => {
            if file_raw.trim().is_empty() {
                bail!("codex returned empty --output-last-message");
            }
            Ok(parse_message(&file_raw))
        }
        Err(err) => {
            if !file_raw.trim().is_empty() {
                Ok(parse_message(&file_raw))
            } else {
                Err(err).context("codex exec")
            }
        }
    }
}

pub fn resolve_cursor_bin(provider: &AiProvider, prefs: &CommitAiPrefs) -> Result<String> {
    if let Some(p) = nonempty_path(&provider.cli_bin) {
        return Ok(p);
    }
    if let Some(p) = nonempty_path(&prefs.cursor_bin) {
        return Ok(p);
    }
    detect_cursor_bin().ok_or_else(|| anyhow!("Cursor CLI not found (agent / cursor-agent)"))
}

pub fn resolve_codex_bin(provider: &AiProvider, prefs: &CommitAiPrefs) -> Result<String> {
    if let Some(p) = nonempty_path(&provider.cli_bin) {
        return Ok(p);
    }
    if let Some(p) = nonempty_path(&prefs.codex_bin) {
        return Ok(p);
    }
    detect_codex_bin().ok_or_else(|| anyhow!("Codex CLI not found (codex)"))
}

pub fn detect_cursor_bin() -> Option<String> {
    super::cli_tools::detect_cursor_bin()
}

pub fn detect_codex_bin() -> Option<String> {
    super::cli_tools::detect_codex_bin()
}

/// Install Cursor Agent or Codex when missing; returns the resolved binary path.
pub fn install_cli_bin(kind: super::cli_tools::CliToolKind) -> Result<String> {
    super::cli_tools::ensure_cli(kind)
}

/// Like [`install_cli_bin`], with 0.0..=1.0 progress callbacks for UI.
pub fn install_cli_bin_with_progress(
    kind: super::cli_tools::CliToolKind,
    progress: super::cli_tools::InstallProgressFn,
) -> Result<String> {
    super::cli_tools::ensure_cli_with_progress(kind, Some(progress))
}

/// GET `{base}/models` (OpenAI-compatible) and return model ids.
pub fn fetch_models(provider: &AiProvider) -> Result<Vec<String>> {
    if provider.base_url.trim().is_empty() {
        bail!("base URL is empty");
    }
    let mut url = provider.models_url();
    let key = provider.resolve_api_key();
    if provider.auth == AiAuthKind::Query {
        let param = if provider.auth_query_param.trim().is_empty() {
            "key"
        } else {
            provider.auth_query_param.trim()
        };
        let Some(key) = key.as_deref() else {
            bail!("API key required for query auth");
        };
        let sep = if url.contains('?') { '&' } else { '?' };
        url = format!("{url}{sep}{param}={}", urlencoding_minimal(key));
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent(&format!("simple-git-gui/{}", env!("CARGO_PKG_VERSION")))
        .build();
    let mut req = agent.get(&url);
    match provider.auth {
        AiAuthKind::None | AiAuthKind::Query => {}
        AiAuthKind::Bearer => {
            let Some(key) = key.as_deref() else {
                bail!("API key required for Bearer auth");
            };
            req = req.set("Authorization", &format!("Bearer {key}"));
        }
        AiAuthKind::Header => {
            let Some(key) = key.as_deref() else {
                bail!("API key required for header auth");
            };
            let header = if provider.auth_header.trim().is_empty() {
                "x-api-key"
            } else {
                provider.auth_header.trim()
            };
            req = req.set(header, key);
        }
    }
    let response = req.call().with_context(|| format!("GET {url}"))?;
    let parsed: ModelsResponse = response.into_json().context("parse /models JSON")?;
    let mut ids: Vec<String> = parsed
        .data
        .into_iter()
        .map(|m| m.id)
        .filter(|id| !id.trim().is_empty())
        .collect();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        bail!("no models returned");
    }
    Ok(ids)
}

#[derive(Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    #[serde(default)]
    id: String,
}

fn nonempty_path(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    Some(t.to_string())
}

fn run_command_capture(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    stdin_text: Option<&str>,
    env: &[(&str, &str)],
    timeout_secs: Option<u64>,
    abort: Option<&AbortFlag>,
) -> Result<String> {
    check_aborted(abort)?;
    // Windows: .cmd/.bat cannot take args via CreateProcess — use command_for.
    let mut cmd = super::cli_tools::command_for(program);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    if stdin_text.is_some() {
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }
    hide_console(&mut cmd);

    let mut child = cmd.spawn().with_context(|| format!("spawn {program}"))?;
    if let Some(text) = stdin_text {
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .context("write prompt to stdin")?;
        }
    }

    let stdout_buf = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf = Arc::new(Mutex::new(Vec::new()));
    let out_h = child.stdout.take().map(|pipe| {
        let buf = stdout_buf.clone();
        thread::spawn(move || {
            let mut pipe = pipe;
            let mut tmp = [0u8; 4096];
            while let Ok(n) = pipe.read(&mut tmp) {
                if n == 0 {
                    break;
                }
                if let Ok(mut g) = buf.lock() {
                    g.extend_from_slice(&tmp[..n]);
                }
            }
        })
    });
    let err_h = child.stderr.take().map(|pipe| {
        let buf = stderr_buf.clone();
        thread::spawn(move || {
            let mut pipe = pipe;
            let mut tmp = [0u8; 4096];
            while let Ok(n) = pipe.read(&mut tmp) {
                if n == 0 {
                    break;
                }
                if let Ok(mut g) = buf.lock() {
                    g.extend_from_slice(&tmp[..n]);
                }
            }
        })
    });

    let started = Instant::now();
    let limit = timeout_secs.filter(|s| *s > 0).map(Duration::from_secs);
    let status = loop {
        check_aborted(abort).map_err(|err| {
            let _ = child.kill();
            let _ = child.wait();
            err
        })?;
        if let Some(limit) = limit {
            if started.elapsed() >= limit {
                let _ = child.kill();
                let _ = child.wait();
                bail!("{program} timed out after {}s", limit.as_secs());
            }
        }
        match child.try_wait().with_context(|| format!("wait {program}"))? {
            Some(status) => break status,
            None => thread::sleep(Duration::from_millis(50)),
        }
    };
    if let Some(h) = out_h {
        let _ = h.join();
    }
    if let Some(h) = err_h {
        let _ = h.join();
    }
    let stdout = String::from_utf8_lossy(&stdout_buf.lock().unwrap_or_else(|e| e.into_inner()))
        .trim()
        .to_string();
    let stderr = String::from_utf8_lossy(&stderr_buf.lock().unwrap_or_else(|e| e.into_inner()))
        .trim()
        .to_string();
    if !status.success() {
        let code = status.code().unwrap_or(-1);
        bail!("{program} exited {code}: {stderr}");
    }
    if stdout.is_empty() && !stderr.is_empty() {
        return Ok(stderr);
    }
    Ok(stdout)
}

fn hide_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}
