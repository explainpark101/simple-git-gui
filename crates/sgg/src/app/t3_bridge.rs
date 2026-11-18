//! Detect T3 Code install / harness and import its enabled agents into sgg.
//!
//! T3 stores provider status under `~/.t3/caches/{driver}.json` and instance
//! prefs in `~/.t3/userdata/settings.json`. Browser-login credentials are
//! encrypted there; we reuse process `CURSOR_API_KEY` when present and sync
//! models / enable matching built-in CLI providers.

use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use super::ai_provider::{AiProviderKind, BUILTIN_CODEX_ID, BUILTIN_CURSOR_ID};
use super::prefs::CommitAiPrefs;

pub const CURSOR_API_KEYS_URL: &str = "https://cursor.com/dashboard?tab=integrations";

#[derive(Debug, Clone)]
pub struct T3Model {
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct T3Agent {
    pub id: String,
    pub display_name: String,
    pub driver: String,
    pub enabled: bool,
    pub status: String,
    pub auth_status: Option<String>,
    pub models: Vec<T3Model>,
}

#[derive(Debug, Clone, Default)]
pub struct T3ImportResult {
    pub cursor_models: Vec<String>,
    pub preferred_cursor_model: Option<String>,
    pub enabled_cursor: bool,
    pub enabled_codex: bool,
    pub used_env_cursor_key: bool,
    pub summary: String,
}

pub fn t3_home() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".t3"))
}

pub fn userdata_dir() -> Option<PathBuf> {
    t3_home().map(|h| h.join("userdata"))
}

pub fn caches_dir() -> Option<PathBuf> {
    t3_home().map(|h| h.join("caches"))
}

/// True when T3 Code userdata (or caches) exists on this machine.
pub fn is_t3_installed() -> bool {
    userdata_dir().is_some_and(|p| p.is_dir()) || caches_dir().is_some_and(|p| p.is_dir())
}

/// True when this process is running under the T3 Code harness / desktop host.
pub fn is_running_under_t3() -> bool {
    std::env::var_os("T3CODE_CLI_PATH").is_some()
        || std::env::var_os("T3CODE_DESKTOP_HOST_SCOPE").is_some()
        || std::env::var_os("T3_CODE").is_some()
}

pub fn t3_available() -> bool {
    is_t3_installed() || is_running_under_t3()
}

#[derive(Debug, Deserialize)]
struct CacheFile {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    auth: Option<CacheAuth>,
    #[serde(default)]
    models: Vec<CacheModel>,
    #[serde(default)]
    instance_id: Option<String>,
    #[serde(default)]
    driver: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CacheAuth {
    #[serde(default)]
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CacheModel {
    #[serde(default)]
    slug: String,
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct T3Settings {
    #[serde(default)]
    text_generation_model_selection: Option<TextGenSelection>,
    #[serde(rename = "textGenerationModelSelection", default)]
    text_generation_model_selection_camel: Option<TextGenSelection>,
    #[serde(default)]
    provider_instances: Option<serde_json::Value>,
    #[serde(rename = "providerInstances", default)]
    provider_instances_camel: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct TextGenSelection {
    #[serde(default)]
    instance_id: Option<String>,
    #[serde(rename = "instanceId", default)]
    instance_id_camel: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

impl TextGenSelection {
    fn instance_id(&self) -> Option<&str> {
        self.instance_id
            .as_deref()
            .or(self.instance_id_camel.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    fn model(&self) -> Option<&str> {
        self.model
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

pub fn list_agents() -> Vec<T3Agent> {
    let Some(dir) = caches_dir() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut agents = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        // Skip nested cache folders' side files if any; only top-level provider caches.
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.contains('/') || stem.contains('\\') {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(cache) = serde_json::from_str::<CacheFile>(&raw) else {
            continue;
        };
        let driver = cache
            .driver
            .clone()
            .unwrap_or_else(|| stem.to_string());
        let id = cache
            .instance_id
            .clone()
            .unwrap_or_else(|| stem.to_string());
        let display_name = cache
            .display_name
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| title_case_driver(&driver));
        let models = cache
            .models
            .into_iter()
            .filter(|m| !m.slug.trim().is_empty())
            .map(|m| T3Model {
                name: if m.name.trim().is_empty() {
                    m.slug.clone()
                } else {
                    m.name
                },
                slug: m.slug,
            })
            .collect();
        agents.push(T3Agent {
            id,
            display_name,
            driver,
            enabled: cache.enabled,
            status: cache.status.unwrap_or_else(|| "unknown".into()),
            auth_status: cache.auth.and_then(|a| a.status),
            models,
        });
    }
    agents.sort_by(|a, b| {
        b.enabled
            .cmp(&a.enabled)
            .then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
    });
    agents
}

fn title_case_driver(driver: &str) -> String {
    match driver {
        "cursor" => "Cursor".into(),
        "codex" => "Codex".into(),
        "claudeAgent" => "Claude".into(),
        "opencode" => "OpenCode".into(),
        "antigravity" => "Antigravity".into(),
        other => other.to_string(),
    }
}

fn read_preferred_cursor_model() -> Option<String> {
    let path = userdata_dir()?.join("settings.json");
    let raw = fs::read_to_string(path).ok()?;
    let settings: T3Settings = serde_json::from_str(&raw).ok()?;
    let sel = settings
        .text_generation_model_selection
        .or(settings.text_generation_model_selection_camel)?;
    let instance = sel.instance_id()?;
    if !instance.eq_ignore_ascii_case("cursor") {
        return None;
    }
    sel.model().map(|s| s.to_string())
}

fn instance_enabled_in_settings(driver: &str) -> Option<bool> {
    let path = userdata_dir()?.join("settings.json");
    let raw = fs::read_to_string(path).ok()?;
    let settings: T3Settings = serde_json::from_str(&raw).ok()?;
    let value = settings
        .provider_instances
        .or(settings.provider_instances_camel)?;
    let obj = value.as_object()?;
    let entry = obj.get(driver)?;
    entry.get("enabled").and_then(|v| v.as_bool())
}

/// Import enabled T3 agents that map to sgg built-in CLIs (Cursor / Codex).
/// Also returns Cursor model ids for the Settings picker.
pub fn import_agents(prefs: &mut CommitAiPrefs) -> T3ImportResult {
    prefs.ensure_builtin_cli_providers();
    let agents = list_agents();
    let mut result = T3ImportResult::default();

    let cursor = agents.iter().find(|a| {
        a.driver.eq_ignore_ascii_case("cursor") || a.id.eq_ignore_ascii_case("cursor")
    });
    let codex = agents.iter().find(|a| {
        a.driver.eq_ignore_ascii_case("codex") || a.id.eq_ignore_ascii_case("codex")
    });

    let cursor_enabled = cursor
        .map(|a| a.enabled)
        .or_else(|| instance_enabled_in_settings("cursor"))
        .unwrap_or(false);
    let codex_enabled = codex
        .map(|a| a.enabled)
        .or_else(|| instance_enabled_in_settings("codex"))
        .unwrap_or(false);

    if let Some(agent) = cursor {
        result.cursor_models = agent
            .models
            .iter()
            .map(|m| m.slug.clone())
            .filter(|s| !s.is_empty())
            .collect();
    }
    result.preferred_cursor_model = read_preferred_cursor_model().or_else(|| {
        result
            .cursor_models
            .iter()
            .find(|m| *m != "default")
            .cloned()
            .or_else(|| result.cursor_models.first().cloned())
    });

    if let Some(p) = prefs.provider_by_id_mut(BUILTIN_CURSOR_ID) {
        if cursor_enabled || cursor.is_some() {
            p.enabled = true;
            result.enabled_cursor = true;
        }
        if p.api_key_env.trim().is_empty() {
            p.api_key_env = "CURSOR_API_KEY".into();
        }
        if p.api_key.trim().is_empty() {
            if let Ok(key) = std::env::var("CURSOR_API_KEY") {
                let key = key.trim().to_string();
                if !key.is_empty() {
                    // Keep using env fallback — mark that harness already has a key.
                    result.used_env_cursor_key = true;
                }
            }
        }
        if p.model.trim().is_empty() {
            if let Some(model) = &result.preferred_cursor_model {
                if model != "default" {
                    p.model = model.clone();
                }
            }
        }
        if p.kind != AiProviderKind::CursorCli {
            p.kind = AiProviderKind::CursorCli;
        }
    }

    if let Some(p) = prefs.provider_by_id_mut(BUILTIN_CODEX_ID) {
        if codex_enabled || codex.is_some_and(|a| a.enabled) {
            p.enabled = true;
            result.enabled_codex = true;
        }
    }

    let enabled_names: Vec<String> = agents
        .iter()
        .filter(|a| a.enabled)
        .map(|a| {
            let detail = a
                .auth_status
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(a.status.as_str());
            format!("{} [{}]", a.display_name, detail)
        })
        .collect();
    let mut parts = Vec::new();
    if !enabled_names.is_empty() {
        parts.push(format!("T3 agents: {}", enabled_names.join(", ")));
    } else if !agents.is_empty() {
        parts.push(format!("Found {} T3 provider cache(s)", agents.len()));
    } else {
        parts.push("No T3 provider caches found".into());
    }
    // Keep model display names available for future UI (slug is what we store).
    let _ = cursor.map(|a| {
        a.models
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>()
    });
    if result.enabled_cursor {
        parts.push(format!(
            "Cursor linked ({} models)",
            result.cursor_models.len()
        ));
    }
    if result.enabled_codex {
        parts.push("Codex enabled".into());
    }
    if result.used_env_cursor_key {
        parts.push("CURSOR_API_KEY present in environment".into());
    }
    result.summary = parts.join(" · ");
    result
}
