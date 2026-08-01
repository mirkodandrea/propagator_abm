//! Talking to a model, over a blocking socket, one request at a time.
//!
//! Both providers stream, and both are read the same way: a blocking `Read`
//! over the response body, split into lines, each line turned into zero or one
//! deltas by a small pure function. Those two functions —
//! [`openrouter_delta`] and [`ollama_delta`] — are where every format quirk
//! lives and are the only part of this module that can be tested without a
//! server, so they are deliberately the only part that has any logic in it.
//!
//! **Streaming, because an interview is a conversation.** A non-streaming call
//! is simpler and would have been defensible, but a wildfire interview asks a
//! question and then sits on a blank panel for ten seconds, which reads
//! exactly like the thing has hung. The delta callback is what lets the chat
//! window fill in as the answer arrives.
//!
//! **Nothing here knows about Bevy or threads.** [`Client::complete`] blocks
//! the calling thread until the model is done, and it is `game`'s worker
//! thread that makes that acceptable — the same shape `crate::api` already
//! uses for the control API, and for the same reason: the main thread never
//! waits on a socket.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::{LlmConfig, Provider};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    pub fn wire(self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }

    pub fn from_wire(s: &str) -> Option<Role> {
        match s {
            "system" => Some(Role::System),
            "user" => Some(Role::User),
            "assistant" => Some(Role::Assistant),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Message {
        Message { role: Role::System, content: content.into() }
    }

    pub fn user(content: impl Into<String>) -> Message {
        Message { role: Role::User, content: content.into() }
    }

    pub fn assistant(content: impl Into<String>) -> Message {
        Message { role: Role::Assistant, content: content.into() }
    }
}

/// One configured provider, ready to answer.
pub struct Client {
    config: LlmConfig,
}

impl Client {
    pub fn new(config: LlmConfig) -> Client {
        Client { config }
    }

    /// Send `messages` and stream the reply, calling `on_delta` with each
    /// fragment as it arrives. Returns the whole reply.
    ///
    /// Blocking, and slow by design — this is called on a worker thread.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn complete(
        &self,
        messages: &[Message],
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<String> {
        self.config.readiness().map_err(|e| anyhow!(e))?;
        match self.config.provider {
            Provider::OpenRouter => self.openrouter(messages, on_delta),
            Provider::Ollama => self.ollama(messages, on_delta),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn complete(
        &self,
        _messages: &[Message],
        _on_delta: &mut dyn FnMut(&str),
    ) -> Result<String> {
        Err(anyhow!("interviews need a native build: the web build has no HTTP client"))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn openrouter(&self, messages: &[Message], on_delta: &mut dyn FnMut(&str)) -> Result<String> {
        let body = json!({
            "model": self.config.openrouter_model,
            "messages": wire_messages(messages),
            "stream": true,
            "temperature": self.config.temperature,
            "max_tokens": self.config.max_tokens,
        });
        let response = ureq::post("https://openrouter.ai/api/v1/chat/completions")
            .set("Authorization", &format!("Bearer {}", self.config.api_key()))
            .set("Content-Type", "application/json")
            // OpenRouter attributes requests by these two and shows them on the
            // account's activity page. Being identifiable there is the polite
            // thing for a client that spends someone's credit.
            .set("HTTP-Referer", "https://github.com/mirkodandrea/propagator_abm")
            .set("X-Title", "Spotorno wildfire incident")
            .timeout(std::time::Duration::from_secs(120))
            .send_json(body)
            .map_err(describe)?;
        read_stream(response.into_reader(), on_delta, openrouter_delta)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn ollama(&self, messages: &[Message], on_delta: &mut dyn FnMut(&str)) -> Result<String> {
        let url = format!("{}/api/chat", self.config.ollama_url.trim_end_matches('/'));
        let body = json!({
            "model": self.config.ollama_model,
            "messages": wire_messages(messages),
            "stream": true,
            "options": {
                "temperature": self.config.temperature,
                "num_predict": self.config.max_tokens,
            },
        });
        let response = ureq::post(&url)
            .set("Content-Type", "application/json")
            // A local model on a cold start can spend a minute loading weights
            // before it says anything at all.
            .timeout(std::time::Duration::from_secs(300))
            .send_json(body)
            .map_err(describe)?;
        read_stream(response.into_reader(), on_delta, ollama_delta)
    }
}

fn wire_messages(messages: &[Message]) -> Vec<Value> {
    messages
        .iter()
        .map(|m| json!({"role": m.role.wire(), "content": m.content}))
        .collect()
}

/// Turn a transport failure into something a chat window can print.
///
/// `ureq`'s own `Display` for an HTTP error is the status line and nothing
/// else, which for a rejected key reads "http status 401" and leaves the user
/// to guess. Both providers put a real message in the body; this digs it out.
#[cfg(not(target_arch = "wasm32"))]
fn describe(e: ureq::Error) -> anyhow::Error {
    match e {
        ureq::Error::Status(code, response) => {
            let body = response.into_string().unwrap_or_default();
            let detail = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| {
                    v.get("error")
                        .and_then(|e| e.get("message").or(Some(e)))
                        .map(|m| m.as_str().map(str::to_string).unwrap_or_else(|| m.to_string()))
                })
                .unwrap_or_else(|| body.chars().take(300).collect());
            match code {
                401 | 403 => anyhow!("the model provider rejected the API key ({code}): {detail}"),
                404 => anyhow!("no such model ({code}): {detail}"),
                429 => anyhow!("rate limited by the provider ({code}): {detail}"),
                _ => anyhow!("provider returned {code}: {detail}"),
            }
        }
        ureq::Error::Transport(t) => {
            anyhow!("could not reach the model provider: {t}. For Ollama, check `ollama serve` is running.")
        }
    }
}

/// Read a streaming body line by line, feeding each line to `parse` and each
/// resulting fragment to `on_delta`.
#[cfg(not(target_arch = "wasm32"))]
fn read_stream(
    reader: impl std::io::Read,
    on_delta: &mut dyn FnMut(&str),
    parse: fn(&str) -> Chunk,
) -> Result<String> {
    use std::io::BufRead;

    let mut out = String::new();
    let reader = std::io::BufReader::new(reader);
    for line in reader.lines() {
        let line = line.context("reading the model's reply")?;
        match parse(&line) {
            Chunk::Skip => {}
            Chunk::Done => break,
            Chunk::Error(e) => return Err(anyhow!(e)),
            Chunk::Text(t) => {
                out.push_str(&t);
                on_delta(&t);
            }
        }
    }
    if out.trim().is_empty() {
        return Err(anyhow!("the model returned an empty reply"));
    }
    Ok(out)
}

/// What one line of a streaming body turned out to be.
#[derive(Debug, PartialEq)]
pub enum Chunk {
    Text(String),
    /// A keep-alive, a comment, a blank line, or a frame carrying no text.
    Skip,
    /// The provider said the stream is over.
    Done,
    /// The provider reported a failure mid-stream, which is not the same as a
    /// failed request: the HTTP status was 200 and the error is in the body.
    Error(String),
}

/// One `data:` line of an OpenAI-compatible SSE stream.
pub fn openrouter_delta(line: &str) -> Chunk {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        // SSE comments are how OpenRouter keeps a slow connection alive while
        // a request queues; treating one as an empty answer would end the
        // stream before the model had said anything.
        return Chunk::Skip;
    }
    let Some(data) = line.strip_prefix("data:") else {
        return Chunk::Skip;
    };
    let data = data.trim();
    if data == "[DONE]" {
        return Chunk::Done;
    }
    let Ok(v) = serde_json::from_str::<Value>(data) else {
        return Chunk::Skip;
    };
    if let Some(err) = v.get("error") {
        let msg = err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("the provider reported an error mid-stream");
        return Chunk::Error(msg.to_string());
    }
    match v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"))
        .and_then(|d| d.get("content"))
        .and_then(Value::as_str)
    {
        Some(t) if !t.is_empty() => Chunk::Text(t.to_string()),
        _ => Chunk::Skip,
    }
}

/// One line of Ollama's newline-delimited JSON stream.
pub fn ollama_delta(line: &str) -> Chunk {
    let line = line.trim();
    if line.is_empty() {
        return Chunk::Skip;
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Chunk::Skip;
    };
    if let Some(err) = v.get("error").and_then(Value::as_str) {
        return Chunk::Error(err.to_string());
    }
    let text = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("");
    // `done` arrives on a frame that usually carries no content of its own,
    // but Ollama is entitled to put the last token on it — so the text is
    // taken first and the stream ends after.
    if !text.is_empty() {
        return Chunk::Text(text.to_string());
    }
    if v.get("done").and_then(Value::as_bool).unwrap_or(false) {
        return Chunk::Done;
    }
    Chunk::Skip
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openrouter_stream_yields_text_then_done() {
        assert_eq!(
            openrouter_delta(r#"data: {"choices":[{"delta":{"content":"Sto"}}]}"#),
            Chunk::Text("Sto".into())
        );
        assert_eq!(openrouter_delta("data: [DONE]"), Chunk::Done);
    }

    #[test]
    fn openrouter_keepalives_and_role_frames_are_skipped() {
        // A comment line, the opening frame that carries only the role, and a
        // blank line between events. Reading any of these as an end-of-stream
        // truncates the answer to nothing.
        assert_eq!(openrouter_delta(": OPENROUTER PROCESSING"), Chunk::Skip);
        assert_eq!(
            openrouter_delta(r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#),
            Chunk::Skip
        );
        assert_eq!(openrouter_delta(""), Chunk::Skip);
    }

    #[test]
    fn an_error_inside_a_200_stream_is_reported() {
        // The request succeeded and the failure is in the body — without this
        // the interview would simply stop mid-sentence with no explanation.
        let c = openrouter_delta(r#"data: {"error":{"message":"upstream is down"}}"#);
        assert_eq!(c, Chunk::Error("upstream is down".into()));
        assert_eq!(
            ollama_delta(r#"{"error":"model 'nope' not found"}"#),
            Chunk::Error("model 'nope' not found".into())
        );
    }

    #[test]
    fn ollama_stream_yields_text_then_done() {
        assert_eq!(
            ollama_delta(r#"{"message":{"role":"assistant","content":"Ho"},"done":false}"#),
            Chunk::Text("Ho".into())
        );
        assert_eq!(
            ollama_delta(r#"{"message":{"role":"assistant","content":""},"done":true}"#),
            Chunk::Done
        );
    }

    #[test]
    fn ollamas_last_token_can_ride_on_the_done_frame() {
        // Text first, `done` after: the other order drops the final token, and
        // a dropped last token is invisible in a chat window.
        assert_eq!(
            ollama_delta(r#"{"message":{"content":"paura."},"done":true}"#),
            Chunk::Text("paura.".into())
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_whole_stream_reassembles_in_order() {
        let body = concat!(
            ": keep-alive\n",
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n",
            "\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Ho visto \"}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"il fumo.\"}}]}\n",
            "data: [DONE]\n",
        );
        let mut seen = Vec::new();
        let out = read_stream(body.as_bytes(), &mut |d| seen.push(d.to_string()), openrouter_delta)
            .unwrap();
        assert_eq!(out, "Ho visto il fumo.");
        assert_eq!(seen, vec!["Ho visto ", "il fumo."]);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn an_empty_stream_is_an_error_not_an_empty_answer() {
        // An empty assistant message saved to the transcript looks exactly
        // like an agent refusing to speak, which is a behaviour the model is
        // allowed to have — so the transport failure has to be told apart from
        // it here, where it is still distinguishable.
        let out = read_stream("data: [DONE]\n".as_bytes(), &mut |_| {}, openrouter_delta);
        assert!(out.is_err());
    }
}
