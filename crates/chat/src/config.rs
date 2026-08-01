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
pub fn load_dotenv() -> Option<PathBuf> {
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
                return Some(candidate);
            }
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

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
    pub fn load() -> Result<LlmConfig> {
        let path = config_path();
        if !path.exists() {
            return Ok(LlmConfig::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
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

/// `~/.config/spotorno/llm.json`, or `SPOTORNO_LLM_CONFIG`.
pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("SPOTORNO_LLM_CONFIG") {
        return PathBuf::from(p);
    }
    config_dir().join("llm.json")
}

fn home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."))
}

/// Hand-rolled rather than pulling in `dirs`: two paths, one platform family,
/// and the XDG variables are the whole of the rule anyone here would expect.
fn config_dir() -> PathBuf {
    match std::env::var("XDG_CONFIG_HOME") {
        Ok(p) if !p.is_empty() => PathBuf::from(p).join("spotorno"),
        _ => home().join(".config").join("spotorno"),
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
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
