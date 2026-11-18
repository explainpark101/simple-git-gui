//! AI-assisted commit message generation (Changes view).
//!
//! Uses the multi-provider registry in Settings. Auto tries enabled providers
//! in list order. Never commits — fills the commit textarea for review.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sgg_core::{prepare_commit_context, CommitMessageContext};

use super::ai_provider::{AiAuthKind, AiProvider, AiProviderKind};
use super::prefs::{CommitAiPrefs, CommitAiStyle};

const HTTP_TIMEOUT_SECS: u64 = 120;
const CLI_TIMEOUT_SECS: u64 = 180;

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
pub fn generate_for_repo(
    repo: &Path,
    user_hint: Option<&str>,
    prefs: &CommitAiPrefs,
) -> Result<GeneratedCommitMessage> {
    let ctx = prepare_commit_context(repo, user_hint).context("prepare commit context")?;
    if ctx.is_empty() {
        bail!("no staged or unstaged changes to describe");
    }
    generate_from_context(repo, &ctx, prefs)
}

fn generate_from_context(
    repo: &Path,
    ctx: &CommitMessageContext,
    prefs: &CommitAiPrefs,
) -> Result<GeneratedCommitMessage> {
    let prompt = build_prompt(ctx, prefs.style);
    if prefs.is_auto() {
        let mut errors = Vec::new();
        let mut tried = 0usize;
        for provider in prefs.providers.iter().filter(|p| p.enabled) {
            if !provider_usable(provider, prefs) {
                continue;
            }
            tried += 1;
            match generate_with_provider(repo, &prompt, provider, prefs) {
                Ok(msg) => return Ok(msg),
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
    generate_with_provider(repo, &prompt, provider, prefs)
}

fn provider_usable(provider: &AiProvider, prefs: &CommitAiPrefs) -> bool {
    match provider.kind {
        AiProviderKind::OpenAiCompatible => provider.openai_ready(),
        AiProviderKind::CursorCli => resolve_cursor_bin(prefs).is_ok(),
        AiProviderKind::CodexCli => resolve_codex_bin(prefs).is_ok(),
    }
}

fn generate_with_provider(
    repo: &Path,
    prompt: &str,
    provider: &AiProvider,
    prefs: &CommitAiPrefs,
) -> Result<GeneratedCommitMessage> {
    match provider.kind {
        AiProviderKind::OpenAiCompatible => generate_http(prompt, provider),
        AiProviderKind::CursorCli => {
            let bin = resolve_cursor_bin(prefs)?;
            generate_cursor_with_bin(repo, prompt, &bin)
        }
        AiProviderKind::CodexCli => {
            let bin = resolve_codex_bin(prefs)?;
            generate_codex_with_bin(prompt, &bin)
        }
    }
}

fn build_prompt(ctx: &CommitMessageContext, style: CommitAiStyle) -> String {
    let style_rules = match style {
        CommitAiStyle::Conventional => {
            "Use Conventional Commits: type(optional-scope): subject.\n\
             Types: feat, fix, docs, style, refactor, test, chore.\n\
             Subject: imperative mood, ~50 chars, no trailing period."
        }
        CommitAiStyle::Free => {
            "Write a clear imperative subject (~50–72 chars). Optional short body."
        }
    };

    let mut out = String::new();
    out.push_str(
        "You write git commit messages. Output ONLY the commit message text \
         (subject, then optional blank line + body). No markdown fences, no commentary.\n",
    );
    out.push_str(style_rules);
    out.push('\n');
    out.push_str(
        "Do not run git commands. Do not invent files not in the diff. \
         Avoid secrets; never paste API keys or passwords into the message.\n\n",
    );
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

fn parse_message(raw: &str) -> GeneratedCommitMessage {
    let text = strip_fences(raw).trim().to_string();
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

fn generate_http(prompt: &str, provider: &AiProvider) -> Result<GeneratedCommitMessage> {
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

    let body = ChatRequest {
        model: provider.model.trim().to_string(),
        temperature: 0.2,
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
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .user_agent(&format!("simple-git-gui/{}", env!("CARGO_PKG_VERSION")))
        .build();
    let mut req = agent.post(&url).set("Content-Type", "application/json");
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
    let response = req
        .send_json(&body)
        .with_context(|| format!("POST {url}"))?;
    let parsed: ChatResponse = response.into_json().context("parse chat completions JSON")?;
    let content = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message)
        .map(|m| m.content)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("empty model response"))?;
    Ok(parse_message(&content))
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

// --- Cursor / Codex CLI -------------------------------------------------------

fn generate_cursor_with_bin(
    repo: &Path,
    prompt: &str,
    bin: &str,
) -> Result<GeneratedCommitMessage> {
    let output = run_command_capture(
        bin,
        &["-p", prompt, "--output-format", "text"],
        Some(repo),
        None,
        CLI_TIMEOUT_SECS,
    )?;
    if output.trim().is_empty() {
        bail!("cursor agent returned empty output");
    }
    Ok(parse_message(&output))
}

fn generate_codex_with_bin(prompt: &str, bin: &str) -> Result<GeneratedCommitMessage> {
    let output = run_command_capture(
        bin,
        &[
            "exec",
            "--ephemeral",
            "--skip-git-repo-check",
            "-s",
            "read-only",
            "-",
        ],
        None,
        Some(prompt),
        CLI_TIMEOUT_SECS,
    )?;
    if output.trim().is_empty() {
        bail!("codex returned empty output");
    }
    Ok(parse_message(&output))
}

pub fn resolve_cursor_bin(prefs: &CommitAiPrefs) -> Result<String> {
    if let Some(p) = nonempty_path(&prefs.cursor_bin) {
        return Ok(p);
    }
    which_first(&["agent", "cursor-agent"])
        .ok_or_else(|| anyhow!("Cursor CLI not found (agent / cursor-agent)"))
}

pub fn resolve_codex_bin(prefs: &CommitAiPrefs) -> Result<String> {
    if let Some(p) = nonempty_path(&prefs.codex_bin) {
        return Ok(p);
    }
    which_first(&["codex"]).ok_or_else(|| anyhow!("Codex CLI not found (codex)"))
}

fn nonempty_path(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    Some(t.to_string())
}

fn which_first(names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(p) = which_bin(name) {
            return Some(p);
        }
    }
    None
}

fn which_bin(name: &str) -> Option<String> {
    #[cfg(windows)]
    let finder = "where";
    #[cfg(not(windows))]
    let finder = "which";

    let mut cmd = Command::new(finder);
    cmd.arg(name)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_console(&mut cmd);
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout.lines().next()?.trim();
    if first.is_empty() {
        return None;
    }
    Some(first.to_string())
}

fn run_command_capture(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    stdin_text: Option<&str>,
    _timeout_secs: u64,
) -> Result<String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
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
    let output = child
        .wait_with_output()
        .with_context(|| format!("wait {program}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
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
