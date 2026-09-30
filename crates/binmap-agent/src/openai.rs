//! The OpenAI-compatible backend (`A1.1`, `DESIGN-AI §6.1`).
//!
//! One implementation covering OpenAI, DeepSeek, Kimi, Z.ai and every local
//! runner, which is the whole reason the provider table exists.
//!
//! The translation is separated from the transport on purpose. Everything that
//! can be wrong about talking to one of these endpoints — a tool schema
//! rendered the wrong way, arguments that arrive as a JSON *string* rather
//! than an object, a truncated response parsed as though it were complete, a
//! quirk applied to the wrong provider — is in the translation, and none of it
//! needs a network to test. `HttpTransport` is the seam: the tests drive a
//! recorded transport, so the wire format is covered on a machine with no
//! model runner installed at all.

use crate::backend::{
    CompletionRequest, CompletionResponse, FinishReason, IdentifiedCall, ModelBackend, Role, Usage,
};
use crate::provider::{Capabilities, ModelSpec, ProviderSpec, Quirks};
use crate::registry::{Tool, ToolCall};
use binmap_core::error::{Error, Result};
use serde_json::{Value, json};

/// Somewhere to send a POST.
///
/// Exists so the wire format can be tested without a network, and so the real
/// HTTP client is a leaf rather than something the loop depends on.
pub trait HttpTransport: Send + Sync + std::fmt::Debug {
    /// POST `body` to `url` with `headers`, and return the response body.
    ///
    /// Headers rather than a bare key because the two API shapes authenticate
    /// differently: OpenAI-compatible endpoints take `Authorization: Bearer`,
    /// and Anthropic takes `x-api-key` plus a required `anthropic-version`.
    /// Pushing that difference into the backends keeps the transport a
    /// transport.
    ///
    /// Implementations must not log a header value: one of them is a secret
    /// every time. Nothing here ever returns one (`§6.3`).
    fn post_json(&self, url: &str, headers: &[(&str, String)], body: &Value) -> Result<String>;
}

/// A transport that refuses.
///
/// The default, so that a backend built on a machine with no HTTP client
/// configured fails with a sentence rather than appearing to work.
#[derive(Debug, Clone, Default)]
pub struct UnavailableTransport;

impl HttpTransport for UnavailableTransport {
    fn post_json(&self, url: &str, _headers: &[(&str, String)], _body: &Value) -> Result<String> {
        Err(Error::Other(format!(
            "no HTTP transport is configured, so {url} cannot be reached. A reasoner needs one; \
             every deterministic analysis does not."
        )))
    }
}

/// An OpenAI-compatible endpoint.
#[derive(Debug)]
pub struct OpenAiCompatibleBackend {
    id: String,
    base_url: String,
    key_env: &'static str,
    model: String,
    capabilities: Capabilities,
    quirks: Quirks,
    transport: std::sync::Arc<dyn HttpTransport>,
}

impl OpenAiCompatibleBackend {
    pub fn new(
        spec: &ProviderSpec,
        model: &ModelSpec,
        transport: std::sync::Arc<dyn HttpTransport>,
    ) -> Self {
        Self {
            id: format!("{}/{}", spec.id, model.id),
            base_url: spec.default_base_url.to_string(),
            key_env: spec.key_env,
            model: model.id.to_string(),
            capabilities: model.capabilities.clone(),
            quirks: spec.quirks,
            transport,
        }
    }

    /// Point at a different endpoint — a local runner on another port, or a
    /// proxy.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// The key, read at the moment of use and never stored on this struct.
    ///
    /// `§6.3` says keys belong in the OS keyring with the environment as a
    /// fallback. Only the fallback is wired here; the keyring is its own
    /// change, and holding the key in a field would make that change a
    /// refactor of everything that touches this type.
    fn key(&self) -> Option<String> {
        std::env::var(self.key_env).ok().filter(|key| !key.trim().is_empty())
    }
}

impl ModelBackend for OpenAiCompatibleBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let request = apply_quirks(request, &self.quirks);
        let body = encode_request(&request, &self.model);
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        // OpenAI-compatible endpoints authenticate with a bearer token. A
        // local runner needs none, and sending an empty one is worse than
        // sending nothing.
        let mut headers: Vec<(&str, String)> = vec![("content-type", "application/json".into())];
        if let Some(key) = self.key() {
            headers.push(("authorization", format!("Bearer {key}")));
        }
        let raw = self.transport.post_json(&url, &headers, &body)?;
        decode_response(&raw)
    }
}

/// Reshape a request for what a provider will actually accept (`§6.2`).
pub fn apply_quirks(mut request: CompletionRequest, quirks: &Quirks) -> CompletionRequest {
    if quirks.no_system_prompt {
        request = request.without_system_role();
    }
    if quirks.fixed_temperature {
        request.temperature = None;
    }
    request
}

/// A tool, as a function declaration.
///
/// The schema comes from the registry unchanged — the same string an MCP
/// client would see. Two renderings of one tool is how the descriptions drift
/// apart, and `§4.1` is explicit that the tool is described once.
pub fn encode_tool(tool: &Tool) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters,
        }
    })
}

/// The request body.
pub fn encode_request(request: &CompletionRequest, model: &str) -> Value {
    let messages: Vec<Value> = request
        .messages
        .iter()
        .map(|message| {
            let mut object = json!({
                "role": match message.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => "tool",
                },
                "content": message.content,
            });
            if let Some(id) = &message.tool_call_id {
                object["tool_call_id"] = json!(id);
            }
            object
        })
        .collect();

    let mut body = json!({
        "model": model,
        "messages": messages,
        "max_tokens": request.max_output,
    });

    if !request.tools.is_empty() {
        body["tools"] = Value::Array(request.tools.iter().map(encode_tool).collect());
    }
    if let Some(temperature) = request.temperature {
        body["temperature"] = json!(temperature);
    }
    body
}

/// Parse a response.
///
/// Tolerant in one specific way and strict everywhere else. The tolerance:
/// every one of these endpoints sends tool-call arguments as a JSON *string*
/// containing JSON, and some local runners send an object instead. Both are
/// accepted. The strictness: a body that is not a completion, or a choice with
/// no message, is an error rather than an empty response — an empty response
/// would let the loop treat a broken endpoint as a model with nothing to say.
pub fn decode_response(raw: &str) -> Result<CompletionResponse> {
    let value: Value = serde_json::from_str(raw).map_err(|error| {
        Error::Other(format!("the endpoint returned something that is not JSON: {error}"))
    })?;

    // An error body is the common case when a key is wrong or a model name is
    // misspelled, and it deserves its own message rather than "no choices".
    if let Some(message) = value.pointer("/error/message").and_then(Value::as_str) {
        return Err(Error::Other(format!("the endpoint refused: {message}")));
    }

    let choice = value
        .pointer("/choices/0")
        .ok_or_else(|| Error::Other("the endpoint returned no choices".to_string()))?;
    let message = choice.get("message").ok_or_else(|| {
        Error::Other("the endpoint returned a choice with no message".to_string())
    })?;

    let content = message.get("content").and_then(Value::as_str).unwrap_or_default().to_string();
    // DeepSeek and friends put reasoning in its own field. Absent elsewhere.
    let reasoning = message
        .get("reasoning_content")
        .or_else(|| message.get("reasoning"))
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string);

    let mut calls = Vec::new();
    if let Some(requested) = message.get("tool_calls").and_then(Value::as_array) {
        for (index, call) in requested.iter().enumerate() {
            let name = call
                .pointer("/function/name")
                .and_then(Value::as_str)
                .ok_or_else(|| Error::Other(format!("tool call {index} names no function")))?;
            let arguments = decode_arguments(call.pointer("/function/arguments"))?;
            // A missing id is not fatal: the result has to be matched back to
            // *something*, and an index is a worse id than the provider's own
            // but better than dropping the call.
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("call-{index}"));
            calls.push(IdentifiedCall { id, call: ToolCall { tool: name.to_string(), arguments } });
        }
    }

    let usage = Usage {
        input_tokens: value.pointer("/usage/prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
        output_tokens: value
            .pointer("/usage/completion_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    };

    // Trust the declared reason, but never report `Stop` while holding calls:
    // some endpoints say "stop" and attach tool calls anyway, and a loop that
    // believed them would drop the calls and conclude early.
    let declared = choice.get("finish_reason").and_then(Value::as_str).unwrap_or("stop");
    let finish_reason = match declared {
        "length" | "max_tokens" => FinishReason::Length,
        "tool_calls" | "function_call" => FinishReason::ToolCalls,
        _ if !calls.is_empty() => FinishReason::ToolCalls,
        _ => FinishReason::Stop,
    };

    Ok(CompletionResponse { content, reasoning, calls, usage, finish_reason })
}

/// Arguments, however they arrived.
fn decode_arguments(raw: Option<&Value>) -> Result<Value> {
    match raw {
        // The normal case: a string of JSON.
        Some(Value::String(text)) => {
            if text.trim().is_empty() {
                return Ok(json!({}));
            }
            serde_json::from_str(text).map_err(|error| {
                // This is `Quirks::malformed_tool_arguments` in the wild. The
                // loop retries once with this message fed back, so it has to
                // say what was wrong *and* what arrived.
                Error::Other(format!(
                    "tool arguments were not valid JSON ({error}). Received: {text}"
                ))
            })
        }
        // Some local runners send the object directly.
        Some(object @ Value::Object(_)) => Ok(object.clone()),
        // No arguments at all is legitimate for a tool that takes none.
        None | Some(Value::Null) => Ok(json!({})),
        Some(other) => Err(Error::Other(format!(
            "tool arguments were neither an object nor a JSON string, but {other}"
        ))),
    }
}

/// A real HTTP client (`A1.1`).
///
/// Blocking, matching `ModelBackend`. The timeout is not optional: a local
/// runner that has loaded a model but not finished warming it will accept a
/// connection and then say nothing, and a loop with no timeout waits for it
/// forever while the interface shows "Working…".
#[derive(Debug, Clone)]
pub struct UreqTransport {
    timeout: std::time::Duration,
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqTransport {
    pub fn new() -> Self {
        // Generous, because a local model on CPU is genuinely slow, and a
        // timeout that fires on a working setup is worse than a slow one.
        Self { timeout: std::time::Duration::from_secs(180) }
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl HttpTransport for UreqTransport {
    fn post_json(&self, url: &str, headers: &[(&str, String)], body: &Value) -> Result<String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(self.timeout))
            // A non-2xx is not an error to be thrown away: its body is
            // where the provider says what was wrong — "model not found",
            // "invalid api key" — and that sentence is the one the user
            // needs. Letting ureq turn the status into an error discarded
            // it in favour of the number.
            .http_status_as_error(false)
            .build()
            .into();

        let mut request = agent.post(url);
        for (name, value) in headers {
            request = request.header(*name, value);
        }

        // Serialised here rather than through ureq's `json` feature: we
        // already depend on serde_json, and the body is the thing the tests
        // assert on, so it is better to own it.
        let encoded = serde_json::to_string(body)
            .map_err(|error| Error::Other(format!("could not encode the request: {error}")))?;

        match request.send(&encoded) {
            Ok(mut response) => {
                let status = response.status();
                let body = response.body_mut().read_to_string().map_err(|error| {
                    Error::Other(format!("could not read the response from {url}: {error}"))
                })?;

                if status.is_success() {
                    return Ok(body);
                }

                // Prefer what the provider said over what the status was. A 404
                // from a local runner means the model was never pulled, and its
                // body says so by name — which "HTTP 404" does not.
                let explanation = serde_json::from_str::<Value>(&body)
                    .ok()
                    .and_then(|value| {
                        value
                            .pointer("/error/message")
                            .or_else(|| value.pointer("/error"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .filter(|message| !message.trim().is_empty());

                Err(Error::Other(match explanation {
                    Some(message) => format!("{url} returned HTTP {status}: {message}"),
                    None => format!(
                        "{url} returned HTTP {status}. If this is a local runner, check that the \
                         model is pulled and the port is right."
                    ),
                }))
            }
            Err(error) => Err(Error::Other(format!(
                "could not reach {url}: {error}. A local runner has to be running before it can \
                 be asked anything."
            ))),
        }
    }
}
