//! Which model answers, and where the key for it lives.
//!
//! Two providers, chosen because between them they cover both people who will
//! use this: **OpenRouter** for someone who wants a good model and has a key,
//! **Ollama** for someone who wants no key and no network at all. The rest of
//! the crate talks to whichever through [`crate::provider::Client`] and does
//! not know which it got.
//!
//! **A key is never written into the repository.** The config file lives in the
//! user's own config directory (`~/.config/spotorno/llm.json`, or wherever
//! `SPOTORNO_LLM_CONFIG` points), and the environment always wins over it, so
//! `OPENROUTER_API_KEY=… cargo run` needs no file at all and leaves nothing
//! behind. A file written by [`LlmConfig::save`] is created with owner-only
//! permissions on Unix for the same reason.

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Read a `.env` file into the process environment, without overwriting
/// anything already set there.
///
/// Cargo does not do this, and a key sitting in a `.env` that nothing reads
/// looks exactly like a key the game ignored — which is the failure this is
/// here to prevent. Called once from `main` before anything reads a variable.
///
/// Searches the working directory and up to three parents, so `cargo run` from
/// a crate directory finds the repository root's file. Values may be quoted;
/// `export KEY=value` is accepted because that is what a shell file looks like.
/// **Existing environment variables always win**, so `OPENROUTER_API_KEY=… cargo
/// run` still overrides the file.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_dotenv() -> Option<String> {
    let mut dir = std::env::current_dir().ok()?;
    for _ in 0..4 {
        let candidate = dir.join(".env");
        if candidate.is_file() {
            if let Ok(text) = std::fs::read_to_string(&candidate) {
                for (key, value) in parse_dotenv(&text) {
                    if std::env::var_os(&key).is_none() {
                        std::env::set_var(&key, &value);
                    }
                }
                return Some(candidate.display().to_string());
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
pub fn load_dotenv() -> Option<String> { None }

fn parse_dotenv(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        out.push((key.to_string(), value.to_string()));
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    OpenRouter,
    Ollama,
}

impl Provider {
    pub const ALL: [Provider; 2] = [Provider::OpenRouter, Provider::Ollama];

    pub fn label(self) -> &'static str {
        match self {
            Provider::OpenRouter => "OpenRouter",
            Provider::Ollama => "Ollama (local)",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Provider::OpenRouter => "Hosted models. Needs an API key from openrouter.ai.",
            Provider::Ollama => "A model running on this machine. No key, no network.",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    pub provider: Provider,
    pub openrouter_model: String,
    /// Empty unless the user typed one in. `OPENROUTER_API_KEY` overrides it,
    /// and [`LlmConfig::api_key`] is the only thing that should read either.
    pub openrouter_key: String,
    pub ollama_url: String,
    pub ollama_model: String,
    /// Sampling temperature. A shade above the usual default: this is a person
    /// being interviewed under stress, and a deterministic one reads as a
    /// press release.
    pub temperature: f32,
    /// Hard cap on the reply, in tokens. An interview answer is a few
    /// sentences; the cap is what stops a model that has decided to write an
    /// essay from filling the transcript with one.
    pub max_tokens: u32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        LlmConfig {
            // Ollama first only if there is no key anywhere: the common case
            // for someone who has set `OPENROUTER_API_KEY` is that they meant
            // to use it.
            provider: if std::env::var("OPENROUTER_API_KEY").is_ok() {
                Provider::OpenRouter
            } else {
                Provider::Ollama
            },
            openrouter_model: "anthropic/claude-sonnet-4.5".to_string(),
            openrouter_key: String::new(),
            ollama_url: std::env::var("OLLAMA_HOST")
                .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string()),
            ollama_model: "llama3.1".to_string(),
            temperature: 0.8,
            max_tokens: 600,
        }
    }
}

impl LlmConfig {
    /// Load the saved config, falling back to defaults when there is no file.
    ///
    /// A malformed file is reported rather than silently replaced: someone who
    /// hand-edited their config and made a typo should be told, not quietly
    /// switched back to a default model and billed for it.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load() -> Result<LlmConfig> {
        let path = config_path();
        if !path.exists() {
            return Ok(LlmConfig::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load() -> Result<LlmConfig> {
        let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else {
            return Ok(LlmConfig::default());
        };
        let Some(text) = storage.get_item(STORAGE_KEY)
            .map_err(|e| anyhow::anyhow!(format!("reading browser LLM settings: {e:?}")))? else {
            return Ok(LlmConfig::default());
        };
        serde_json::from_str(&text).context("parsing browser LLM settings")
    }

    /// The same, but never fails: a broken config file must not stop the game
    /// from starting, so the error becomes the second half of the pair and the
    /// settings dialog shows it.
    pub fn load_reported() -> (LlmConfig, Option<String>) {
        match LlmConfig::load() {
            Ok(c) => (c, None),
            Err(e) => (LlmConfig::default(), Some(format!("{e:#}"))),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("creating {}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        restrict_permissions(&path);
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn save(&self) -> Result<()> {
        let storage = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .context("browser storage is unavailable")?;
        storage
            .set_item(STORAGE_KEY, &serde_json::to_string(self)? )
            .map_err(|e| anyhow::anyhow!(format!("saving browser LLM settings: {e:?}")))
    }

    /// The key actually used for a request: the environment first, then the
    /// saved file. Only meaningful for OpenRouter — Ollama has none.
    pub fn api_key(&self) -> String {
        match std::env::var("OPENROUTER_API_KEY") {
            Ok(k) if !k.trim().is_empty() => k,
            _ => self.openrouter_key.clone(),
        }
    }

    /// Whether a request could succeed at all, and what to say if not.
    ///
    /// Answered before anything is sent, so an unconfigured provider is a
    /// sentence in the chat window rather than a connection error thirty
    /// seconds later.
    pub fn readiness(&self) -> Result<(), String> {
        match self.provider {
            Provider::OpenRouter => {
                if self.api_key().trim().is_empty() {
                    Err("OpenRouter needs an API key. Add one in Debug ▸ LLM settings…, \
                         or put OPENROUTER_API_KEY in a .env file at the repository root."
                        .to_string())
                } else if self.model().trim().is_empty() {
                    Err("No OpenRouter model set.".to_string())
                } else {
                    Ok(())
                }
            }
            Provider::Ollama => {
                if self.ollama_url.trim().is_empty() {
                    Err("No Ollama server URL set.".to_string())
                } else if self.model().trim().is_empty() {
                    Err("No Ollama model set. Pull one first, e.g. `ollama pull llama3.1`."
                        .to_string())
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Model id in play, for the request, the status line, and for stamping a
    /// persona with what generated it.
    ///
    /// The environment wins over the saved file, the same way the key does —
    /// so a `.env` naming a model is the model that answers, and the settings
    /// dialog shows that rather than a stale field beside it.
    pub fn model(&self) -> String {
        self.model_from_env().unwrap_or_else(|| match self.provider {
            Provider::OpenRouter => self.openrouter_model.clone(),
            Provider::Ollama => self.ollama_model.clone(),
        })
    }

    /// The model the environment is imposing, if any.
    pub fn model_from_env(&self) -> Option<String> {
        let var = match self.provider {
            Provider::OpenRouter => "OPENROUTER_MODEL",
            Provider::Ollama => "OLLAMA_MODEL",
        };
        std::env::var(var).ok().filter(|v| !v.trim().is_empty())
    }

    /// True when the key came from the environment, so the settings dialog can
    /// say that rather than showing an empty field next to a working provider.
    pub fn key_from_env(&self) -> bool {
        std::env::var("OPENROUTER_API_KEY").map(|k| !k.trim().is_empty()).unwrap_or(false)
    }
}

/// Ask the selected provider for the models it currently exposes.
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_models(config: &LlmConfig) -> Result<Vec<String>> {
    let response = match config.provider {
        Provider::OpenRouter => ureq::get("https://openrouter.ai/api/v1/models")
            .set("Authorization", &format!("Bearer {}", config.api_key()))
            .call()
            .context("requesting OpenRouter models")?,
        Provider::Ollama => ureq::get(&format!("{}/api/tags", config.ollama_url.trim_end_matches('/')))
            .call()
            .context("requesting Ollama models")?,
    };
    let value: serde_json::Value = response.into_json().context("reading provider models")?;
    let mut models: Vec<String> = match config.provider {
        Provider::OpenRouter => value["data"].as_array().into_iter().flatten()
            .filter_map(|m| m["id"].as_str().map(str::to_owned)).collect(),
        Provider::Ollama => value["models"].as_array().into_iter().flatten()
            .filter_map(|m| m["name"].as_str().map(str::to_owned)).collect(),
    };
    models.sort();
    Ok(models)
}

#[cfg(target_arch = "wasm32")]
pub async fn fetch_models(config: &LlmConfig) -> Result<Vec<String>> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, RequestMode, Response};

    let (url, key) = match config.provider {
        Provider::OpenRouter => ("https://openrouter.ai/api/v1/models".to_string(), Some(config.api_key())),
        Provider::Ollama => (format!("{}/api/tags", config.ollama_url.trim_end_matches('/')), None),
    };
    let mut init = RequestInit::new();
    init.set_method("GET");
    init.set_mode(RequestMode::Cors);
    let request = Request::new_with_str_and_init(&url, &init)
        .map_err(|e| anyhow::anyhow!(format!("creating model request: {e:?}")))?;
    if let Some(key) = key.filter(|k| !k.trim().is_empty()) {
        request.headers().set("Authorization", &format!("Bearer {key}"))
            .map_err(|e| anyhow::anyhow!(format!("setting model request headers: {e:?}")))?;
    }
    let window = web_sys::window().context("browser window is unavailable")?;
    let response = JsFuture::from(window.fetch_with_request(&request)).await
        .map_err(|e| anyhow::anyhow!(format!("requesting provider models: {e:?}")))?;
    let response: Response = response.dyn_into().map_err(|e| anyhow::anyhow!(format!("invalid model response: {e:?}")))?;
    if !response.ok() { return Err(anyhow::anyhow!("provider returned HTTP {}", response.status())); }
    let text = JsFuture::from(response.text().map_err(|e| anyhow::anyhow!(format!("reading provider models: {e:?}")))?).await
        .map_err(|e| anyhow::anyhow!(format!("reading provider models: {e:?}")))?;
    let text = text.as_string().ok_or_else(|| anyhow::anyhow!("provider returned a non-text response"))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!(format!("parsing provider models: {e}")))?;
    let mut models: Vec<String> = match config.provider {
        Provider::OpenRouter => value["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str().map(str::to_owned)).collect(),
        Provider::Ollama => value["models"].as_array().into_iter().flatten().filter_map(|m| m["name"].as_str().map(str::to_owned)).collect(),
    };
    models.sort();
    Ok(models)
}

/// `~/.config/spotorno/llm.json`, or `SPOTORNO_LLM_CONFIG`.
#[cfg(not(target_arch = "wasm32"))]
pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("SPOTORNO_LLM_CONFIG") {
        return PathBuf::from(p);
    }
    config_dir().join("llm.json")
}

#[cfg(target_arch = "wasm32")]
const STORAGE_KEY: &str = "spotorno.llm.settings";

pub fn storage_label() -> &'static str {
    #[cfg(target_arch = "wasm32")]
    { "this browser's local storage" }
    #[cfg(not(target_arch = "wasm32"))]
    { "your local config file" }
}

#[cfg(not(target_arch = "wasm32"))]
fn home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."))
}

/// Hand-rolled rather than pulling in `dirs`: two paths, one platform family,
/// and the XDG variables are the whole of the rule anyone here would expect.
#[cfg(not(target_arch = "wasm32"))]
fn config_dir() -> PathBuf {
    match std::env::var("XDG_CONFIG_HOME") {
        Ok(p) if !p.is_empty() => PathBuf::from(p).join("spotorno"),
        _ => home().join(".config").join("spotorno"),
    }
}

#[cfg(all(not(target_arch = "wasm32"), unix))]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(all(not(target_arch = "wasm32"), not(unix)))]
fn restrict_permissions(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openrouter_without_a_key_is_not_ready() {
        let c = LlmConfig {
            provider: Provider::OpenRouter,
            openrouter_key: String::new(),
            ..LlmConfig::default()
        };
        // Only meaningful when the environment is not supplying one; if the
        // developer running the tests has a key exported, the config is ready
        // and that is the correct answer.
        if !c.key_from_env() {
            assert!(c.readiness().is_err());
        }
    }

    #[test]
    fn ollama_needs_no_key() {
        let c = LlmConfig {
            provider: Provider::Ollama,
            ollama_url: "http://127.0.0.1:11434".into(),
            ollama_model: "llama3.1".into(),
            ..LlmConfig::default()
        };
        assert!(c.readiness().is_ok());
    }

    #[test]
    fn config_round_trips_through_json() {
        let c = LlmConfig::default();
        let text = serde_json::to_string(&c).unwrap();
        let back: LlmConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(back.openrouter_model, c.openrouter_model);
        assert_eq!(back.provider, c.provider);
    }

    #[test]
    fn dotenv_lines_parse_the_way_a_shell_file_looks() {
        let pairs = parse_dotenv(
            "# a comment\n\
             \n\
             OPENROUTER_API_KEY=sk-or-abc123\n\
             export OPENROUTER_MODEL=\"anthropic/claude-sonnet-4.5\"\n\
             OLLAMA_HOST='http://localhost:11434'\n\
             not a pair\n",
        );
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], ("OPENROUTER_API_KEY".into(), "sk-or-abc123".into()));
        // `export` prefix stripped, and the quotes with it — both are what a
        // file someone also sources in a shell actually contains.
        assert_eq!(pairs[1], ("OPENROUTER_MODEL".into(), "anthropic/claude-sonnet-4.5".into()));
        assert_eq!(pairs[2].1, "http://localhost:11434");
    }

    #[test]
    fn a_partial_config_file_keeps_the_defaults_for_what_it_omits() {
        // `#[serde(default)]` on the struct: someone hand-editing this file to
        // set one field must not lose the rest.
        let back: LlmConfig = serde_json::from_str(r#"{"ollama_model": "mistral"}"#).unwrap();
        assert_eq!(back.ollama_model, "mistral");
        assert_eq!(back.max_tokens, LlmConfig::default().max_tokens);
    }
}
