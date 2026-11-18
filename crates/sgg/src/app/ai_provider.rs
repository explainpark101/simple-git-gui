//! Multi-provider registry for AI commit messages.
//!
//! OpenAI-compatible entries store title, docs URL, base URL, model, API key,
//! and auth scheme. Cursor / Codex CLI are built-in kinds (stable ids) plus any
//! extra user-added HTTP/CLI providers.

use serde::{Deserialize, Serialize};

/// Stable id for the built-in Cursor Agent CLI provider (T3-style first-class).
pub const BUILTIN_CURSOR_ID: &str = "builtin-cursor";
/// Stable id for the built-in OpenAI Codex CLI provider.
pub const BUILTIN_CODEX_ID: &str = "builtin-codex";

pub fn is_builtin_cli_id(id: &str) -> bool {
    id == BUILTIN_CURSOR_ID || id == BUILTIN_CODEX_ID
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AiProviderKind {
    #[default]
    OpenAiCompatible,
    CursorCli,
    CodexCli,
}

impl AiProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "OpenAI-compatible",
            Self::CursorCli => "Cursor CLI",
            Self::CodexCli => "Codex CLI",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AiAuthKind {
    /// No credential header / query.
    None,
    /// `Authorization: Bearer <api_key>`
    #[default]
    Bearer,
    /// Custom header (default name `x-api-key`).
    Header,
    /// Query string parameter (default name `key`).
    Query,
}

impl AiAuthKind {
    pub const ALL: [Self; 4] = [Self::None, Self::Bearer, Self::Header, Self::Query];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Bearer => "Bearer",
            Self::Header => "Header",
            Self::Query => "Query",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiProvider {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub kind: AiProviderKind,
    /// Documentation / console link shown in Settings.
    #[serde(default)]
    pub docs_url: String,
    /// OpenAI-compatible API root (no trailing slash), e.g. `https://api.openai.com/v1`.
    #[serde(default)]
    pub base_url: String,
    /// Path under base URL. Default `/chat/completions`.
    #[serde(default = "default_chat_path")]
    pub chat_path: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub auth: AiAuthKind,
    /// Header name when [`AiAuthKind::Header`] (default `x-api-key`).
    #[serde(default = "default_auth_header")]
    pub auth_header: String,
    /// Query param when [`AiAuthKind::Query`] (default `key`).
    #[serde(default = "default_auth_query")]
    pub auth_query_param: String,
    /// Optional env var fallback for the API key (e.g. `OPENAI_API_KEY`).
    #[serde(default)]
    pub api_key_env: String,
    /// Absolute path to `agent` / `codex` for CLI providers.
    #[serde(default)]
    pub cli_bin: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Built-in preset slug (`chatgpt`, `deepseek`, …); empty for custom.
    #[serde(default)]
    pub preset: String,
}

fn default_true() -> bool {
    true
}

fn default_chat_path() -> String {
    "/chat/completions".into()
}

/// Quick-pick chat paths for the Settings provider form.
pub const CHAT_PATH_PRESETS: &[&str] = &[
    "/chat/completions",
    "/v1/chat/completions",
    "/completions",
];

fn default_auth_header() -> String {
    "x-api-key".into()
}

fn default_auth_query() -> String {
    "key".into()
}

impl AiProvider {
    pub fn new_id() -> String {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("p-{ms}")
    }

    pub fn blank_http() -> Self {
        Self {
            id: Self::new_id(),
            title: String::new(),
            kind: AiProviderKind::OpenAiCompatible,
            docs_url: String::new(),
            base_url: String::new(),
            chat_path: default_chat_path(),
            model: String::new(),
            api_key: String::new(),
            auth: AiAuthKind::Bearer,
            auth_header: default_auth_header(),
            auth_query_param: default_auth_query(),
            api_key_env: String::new(),
            cli_bin: String::new(),
            enabled: true,
            preset: String::new(),
        }
    }

    pub fn blank_cursor() -> Self {
        let mut p = Self::builtin_cursor();
        p.id = Self::new_id();
        p
    }

    pub fn blank_codex() -> Self {
        let mut p = Self::builtin_codex();
        p.id = Self::new_id();
        p
    }

    pub fn builtin_cursor() -> Self {
        Self {
            id: BUILTIN_CURSOR_ID.into(),
            title: AiProviderPreset::CursorCli.title().into(),
            kind: AiProviderKind::CursorCli,
            // T3 V2 uses @cursor/sdk + CURSOR_API_KEY; we auth the same way and
            // invoke via Cursor's documented headless CLI (`agent -p`).
            docs_url: "https://cursor.com/docs/cli/headless".into(),
            base_url: String::new(),
            chat_path: default_chat_path(),
            model: String::new(),
            api_key: String::new(),
            auth: AiAuthKind::Bearer,
            auth_header: default_auth_header(),
            auth_query_param: default_auth_query(),
            api_key_env: "CURSOR_API_KEY".into(),
            cli_bin: String::new(),
            enabled: true,
            preset: AiProviderPreset::CursorCli.slug().into(),
        }
    }

    pub fn builtin_codex() -> Self {
        Self {
            id: BUILTIN_CODEX_ID.into(),
            title: AiProviderPreset::CodexCli.title().into(),
            kind: AiProviderKind::CodexCli,
            docs_url: "https://developers.openai.com/codex/cli".into(),
            base_url: String::new(),
            chat_path: default_chat_path(),
            model: String::new(),
            api_key: String::new(),
            auth: AiAuthKind::None,
            auth_header: default_auth_header(),
            auth_query_param: default_auth_query(),
            api_key_env: String::new(),
            cli_bin: String::new(),
            enabled: true,
            preset: AiProviderPreset::CodexCli.slug().into(),
        }
    }

    pub fn is_builtin(&self) -> bool {
        is_builtin_cli_id(&self.id)
    }

    pub fn resolve_api_key(&self) -> Option<String> {
        let trimmed = self.api_key.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
        let env_name = self.api_key_env.trim();
        if !env_name.is_empty() {
            if let Ok(v) = std::env::var(env_name) {
                let v = v.trim().to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        None
    }

    pub fn chat_completions_url(&self) -> String {
        let base = self.base_url.trim().trim_end_matches('/');
        let path = self.chat_path.trim();
        let path = if path.is_empty() {
            "/chat/completions"
        } else if path.starts_with('/') {
            path
        } else {
            return format!("{base}/{path}");
        };
        format!("{base}{path}")
    }

    pub fn models_url(&self) -> String {
        let base = self.base_url.trim().trim_end_matches('/');
        format!("{base}/models")
    }

    pub fn openai_ready(&self) -> bool {
        self.kind == AiProviderKind::OpenAiCompatible
            && !self.base_url.trim().is_empty()
            && !self.model.trim().is_empty()
            && (self.auth == AiAuthKind::None || self.resolve_api_key().is_some())
    }
}

/// Cloud / remote OpenAI-compatible presets (autofill the add form only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiProviderPreset {
    ChatGpt,
    GoogleAiStudio,
    DeepSeek,
    LgExaone,
    CursorCli,
    CodexCli,
}

impl AiProviderPreset {
    pub const HTTP_PRESETS: [Self; 4] = [
        Self::ChatGpt,
        Self::GoogleAiStudio,
        Self::DeepSeek,
        Self::LgExaone,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Self::ChatGpt => "chatgpt",
            Self::GoogleAiStudio => "google_ai_studio",
            Self::DeepSeek => "deepseek",
            Self::LgExaone => "lg_exaone",
            Self::CursorCli => "cursor_cli",
            Self::CodexCli => "codex_cli",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::ChatGpt => "ChatGPT API",
            Self::GoogleAiStudio => "Google AI Studio",
            Self::DeepSeek => "DeepSeek API",
            Self::LgExaone => "LG EXAONE API",
            Self::CursorCli => "Cursor CLI",
            Self::CodexCli => "Codex CLI",
        }
    }

    /// Values to drop into the add/edit form (does not register a provider).
    pub fn fill_template(self) -> AiProvider {
        match self {
            Self::ChatGpt => AiProvider {
                id: String::new(),
                title: self.title().into(),
                kind: AiProviderKind::OpenAiCompatible,
                docs_url: "https://platform.openai.com/docs/api-reference".into(),
                base_url: "https://api.openai.com/v1".into(),
                chat_path: default_chat_path(),
                model: "gpt-4o-mini".into(),
                api_key: String::new(),
                auth: AiAuthKind::Bearer,
                auth_header: default_auth_header(),
                auth_query_param: default_auth_query(),
                api_key_env: "OPENAI_API_KEY".into(),
                cli_bin: String::new(),
                enabled: true,
                preset: self.slug().into(),
            },
            Self::GoogleAiStudio => AiProvider {
                id: String::new(),
                title: self.title().into(),
                kind: AiProviderKind::OpenAiCompatible,
                docs_url: "https://ai.google.dev/gemini-api/docs/openai".into(),
                base_url: "https://generativelanguage.googleapis.com/v1beta/openai".into(),
                chat_path: default_chat_path(),
                model: "gemini-2.0-flash".into(),
                api_key: String::new(),
                auth: AiAuthKind::Bearer,
                auth_header: default_auth_header(),
                auth_query_param: default_auth_query(),
                api_key_env: "GOOGLE_API_KEY".into(),
                cli_bin: String::new(),
                enabled: true,
                preset: self.slug().into(),
            },
            Self::DeepSeek => AiProvider {
                id: String::new(),
                title: self.title().into(),
                kind: AiProviderKind::OpenAiCompatible,
                docs_url: "https://api-docs.deepseek.com/".into(),
                base_url: "https://api.deepseek.com".into(),
                chat_path: default_chat_path(),
                model: "deepseek-chat".into(),
                api_key: String::new(),
                auth: AiAuthKind::Bearer,
                auth_header: default_auth_header(),
                auth_query_param: default_auth_query(),
                api_key_env: "DEEPSEEK_API_KEY".into(),
                cli_bin: String::new(),
                enabled: true,
                preset: self.slug().into(),
            },
            Self::LgExaone => AiProvider {
                id: String::new(),
                title: self.title().into(),
                kind: AiProviderKind::OpenAiCompatible,
                docs_url: "https://huggingface.co/LGAI-EXAONE".into(),
                base_url: "https://router.huggingface.co/v1".into(),
                chat_path: default_chat_path(),
                model: "LGAI-EXAONE/EXAONE-3.5-7.8B-Instruct".into(),
                api_key: String::new(),
                auth: AiAuthKind::Bearer,
                auth_header: default_auth_header(),
                auth_query_param: default_auth_query(),
                api_key_env: "HF_TOKEN".into(),
                cli_bin: String::new(),
                enabled: true,
                preset: self.slug().into(),
            },
            Self::CursorCli => AiProvider::blank_cursor(),
            Self::CodexCli => AiProvider::blank_codex(),
        }
    }
}

/// Local OpenAI-compatible servers (autofill base URL only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalLlmPreset {
    Ollama,
    Lemonade,
    LmStudio,
    Custom,
}

impl LocalLlmPreset {
    pub const ALL: [Self; 4] = [Self::Ollama, Self::Lemonade, Self::LmStudio, Self::Custom];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ollama => "Ollama",
            Self::Lemonade => "Lemonade",
            Self::LmStudio => "LM Studio",
            Self::Custom => "Custom",
        }
    }

    pub fn base_url(self) -> &'static str {
        match self {
            Self::Ollama => "http://127.0.0.1:11434/v1",
            Self::Lemonade => "http://127.0.0.1:8000/v1",
            Self::LmStudio => "http://127.0.0.1:1234/v1",
            Self::Custom => "",
        }
    }

    pub fn docs_url(self) -> &'static str {
        match self {
            Self::Ollama => "https://github.com/ollama/ollama/blob/main/docs/openai.md",
            Self::Lemonade => "https://github.com/amd/lemonade",
            Self::LmStudio => "https://lmstudio.ai/docs/app/api/endpoints/openai",
            Self::Custom => "",
        }
    }
}

/// Sentinel active id: try enabled providers in list order.
pub const ACTIVE_AUTO: &str = "auto";

/// Settings form mode — closed until Add / Edit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CommitAiFormMode {
    #[default]
    Closed,
    AddHttp,
    AddCursor,
    AddCodex,
    Edit(String),
}

impl CommitAiFormMode {
    pub fn is_open(&self) -> bool {
        !matches!(self, Self::Closed)
    }

    pub fn edit_id(&self) -> Option<&str> {
        match self {
            Self::Edit(id) => Some(id.as_str()),
            _ => None,
        }
    }
}
