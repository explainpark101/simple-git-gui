//! Multi-provider registry for AI commit messages.
//!
//! OpenAI-compatible entries store title, docs URL, base URL, model, API key,
//! and auth scheme so many endpoints can be registered at once.

use serde::{Deserialize, Serialize};

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

    pub fn openai_ready(&self) -> bool {
        self.kind == AiProviderKind::OpenAiCompatible
            && !self.base_url.trim().is_empty()
            && !self.model.trim().is_empty()
            && (self.auth == AiAuthKind::None || self.resolve_api_key().is_some())
    }
}

/// Built-in catalog entries the user can add (or that ship in defaults).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiProviderPreset {
    ChatGpt,
    GoogleAiStudio,
    DeepSeek,
    LgExaone,
    OllamaLocal,
    CursorCli,
    CodexCli,
}

impl AiProviderPreset {
    pub const HTTP_PRESETS: [Self; 5] = [
        Self::ChatGpt,
        Self::GoogleAiStudio,
        Self::DeepSeek,
        Self::LgExaone,
        Self::OllamaLocal,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Self::ChatGpt => "chatgpt",
            Self::GoogleAiStudio => "google_ai_studio",
            Self::DeepSeek => "deepseek",
            Self::LgExaone => "lg_exaone",
            Self::OllamaLocal => "ollama_local",
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
            Self::OllamaLocal => "Ollama (local)",
            Self::CursorCli => "Cursor CLI",
            Self::CodexCli => "Codex CLI",
        }
    }

    pub fn build(self) -> AiProvider {
        match self {
            Self::ChatGpt => AiProvider {
                id: self.slug().into(),
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
                enabled: true,
                preset: self.slug().into(),
            },
            Self::GoogleAiStudio => AiProvider {
                id: self.slug().into(),
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
                enabled: true,
                preset: self.slug().into(),
            },
            Self::DeepSeek => AiProvider {
                id: self.slug().into(),
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
                enabled: true,
                preset: self.slug().into(),
            },
            Self::LgExaone => AiProvider {
                id: self.slug().into(),
                title: self.title().into(),
                kind: AiProviderKind::OpenAiCompatible,
                // HF OpenAI-compatible router hosts LGAI-EXAONE models.
                docs_url: "https://huggingface.co/LGAI-EXAONE".into(),
                base_url: "https://router.huggingface.co/v1".into(),
                chat_path: default_chat_path(),
                model: "LGAI-EXAONE/EXAONE-3.5-7.8B-Instruct".into(),
                api_key: String::new(),
                auth: AiAuthKind::Bearer,
                auth_header: default_auth_header(),
                auth_query_param: default_auth_query(),
                api_key_env: "HF_TOKEN".into(),
                enabled: true,
                preset: self.slug().into(),
            },
            Self::OllamaLocal => AiProvider {
                id: self.slug().into(),
                title: self.title().into(),
                kind: AiProviderKind::OpenAiCompatible,
                docs_url: "https://github.com/ollama/ollama/blob/main/docs/openai.md".into(),
                base_url: "http://127.0.0.1:11434/v1".into(),
                chat_path: default_chat_path(),
                model: "llama3.2".into(),
                api_key: String::new(),
                auth: AiAuthKind::None,
                auth_header: default_auth_header(),
                auth_query_param: default_auth_query(),
                api_key_env: String::new(),
                enabled: true,
                preset: self.slug().into(),
            },
            Self::CursorCli => AiProvider {
                id: self.slug().into(),
                title: self.title().into(),
                kind: AiProviderKind::CursorCli,
                docs_url: "https://cursor.com/docs/cli/overview".into(),
                base_url: String::new(),
                chat_path: default_chat_path(),
                model: String::new(),
                api_key: String::new(),
                auth: AiAuthKind::None,
                auth_header: default_auth_header(),
                auth_query_param: default_auth_query(),
                api_key_env: String::new(),
                enabled: true,
                preset: self.slug().into(),
            },
            Self::CodexCli => AiProvider {
                id: self.slug().into(),
                title: self.title().into(),
                kind: AiProviderKind::CodexCli,
                docs_url: "https://learn.chatgpt.com/docs/codex/cli.md".into(),
                base_url: String::new(),
                chat_path: default_chat_path(),
                model: String::new(),
                api_key: String::new(),
                auth: AiAuthKind::None,
                auth_header: default_auth_header(),
                auth_query_param: default_auth_query(),
                api_key_env: String::new(),
                enabled: true,
                preset: self.slug().into(),
            },
        }
    }
}

pub fn default_providers() -> Vec<AiProvider> {
    vec![
        AiProviderPreset::ChatGpt.build(),
        AiProviderPreset::GoogleAiStudio.build(),
        AiProviderPreset::DeepSeek.build(),
        AiProviderPreset::LgExaone.build(),
        AiProviderPreset::OllamaLocal.build(),
        AiProviderPreset::CursorCli.build(),
        AiProviderPreset::CodexCli.build(),
    ]
}

/// Sentinel active id: try enabled providers in list order.
pub const ACTIVE_AUTO: &str = "auto";
