//! The Anthropic messages API (`A1.4`, `DESIGN-AI §6.1`).
//!
//! The second of the two shapes the provider table needs, and the reason the
//! table says "the implementation count is two, not five". Everything that
//! differs from OpenAI's chat-completions is structural rather than cosmetic,
//! which is why it cannot be a quirks entry on the other backend:
//!
//! - The system prompt is a **top-level field**, not a message with a role.
//!   Sending it as a message is accepted and ignored, so the failure is a model
//!   that appears not to follow instructions rather than an error.
//! - Content is a **list of blocks**, and a tool call is a block beside the
//!   prose rather than a separate field. A reply can hold both.
//! - Tool results go back as a `tool_result` block inside a **user** turn,
//!   not as a message with a `tool` role.
//! - A tool's schema is `input_schema` at the top level of the tool object,
//!   not nested under `function`.
//! - Authentication is `x-api-key` with a required `anthropic-version`.
//!
//! As with the OpenAI backend the translation is separated from the transport,
//! so all of the above is tested without a network and without a key.

use crate::backend::{
    CompletionRequest, CompletionResponse, FinishReason, IdentifiedCall, ModelBackend, Role, Usage,
};
use crate::openai::HttpTransport;
use crate::provider::{Capabilities, ModelSpec, ProviderSpec};
use crate::registry::{Tool, ToolCall};
use binmap_core::error::{Error, Result};
use serde_json::{Value, json};

/// The version header the API requires. Pinned rather than tracked, because an
/// unannounced change in wire format is exactly what a version header exists
/// to prevent.
pub const API_VERSION: &str = "2023-06-01";

/// Claude, through the messages API.
#[derive(Debug)]
pub struct AnthropicBackend {
    id: String,
    base_url: String,
    key_env: &'static str,
    model: String,
    capabilities: Capabilities,
    transport: std::sync::Arc<dyn HttpTransport>,
}

impl AnthropicBackend {
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
            transport,
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Read at the moment of use and never stored, as in the other backend.
    fn key(&self) -> Option<String> {
        std::env::var(self.key_env).ok().filter(|key| !key.trim().is_empty())
    }
}

impl ModelBackend for AnthropicBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let body = encode_request(&request, &self.model);
        let url = format!("{}/messages", self.base_url.trim_end_matches('/'));
        // `x-api-key`, not a bearer token, and `anthropic-version` is
        // required rather than optional — an unannounced wire change is
        // exactly what a version header exists to prevent.
        let mut headers: Vec<(&str, String)> = vec![
            ("content-type", "application/json".into()),
            ("anthropic-version", API_VERSION.into()),
        ];
        if let Some(key) = self.key() {
            headers.push(("x-api-key", key));
        }
        let raw = self.transport.post_json(&url, &headers, &body)?;
        decode_response(&raw)
    }
}

/// A tool, in Anthropic's shape.
///
/// The schema is the registry's, unchanged — the same string an MCP client
/// sees. What differs from OpenAI is only where it sits in the envelope.
pub fn encode_tool(tool: &Tool) -> Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "input_schema": tool.parameters,
    })
}

/// The request body.
pub fn encode_request(request: &CompletionRequest, model: &str) -> Value {
    // The system prompt is a top-level field. Left as a message it is accepted
    // and ignored, and the symptom is a model that will not follow
    // instructions rather than an error anyone can act on.
    let system: Vec<&str> = request
        .messages
        .iter()
        .filter(|message| message.role == Role::System)
        .map(|message| message.content.as_str())
        .collect();

    let mut messages: Vec<Value> = Vec::new();
    for message in request.messages.iter().filter(|m| m.role != Role::System) {
        match message.role {
            // A tool result is a block inside a *user* turn. There is no
            // `tool` role here.
            Role::Tool => {
                let block = json!({
                    "type": "tool_result",
                    "tool_use_id": message.tool_call_id.clone().unwrap_or_default(),
                    "content": message.content,
                });
                // Consecutive results belong in one user turn, because the API
                // rejects two user turns in a row.
                match messages.last_mut() {
                    Some(last) if last["role"] == "user" && last["content"].is_array() => {
                        last["content"].as_array_mut().expect("checked").push(block);
                    }
                    _ => messages.push(json!({"role": "user", "content": [block]})),
                }
            }
            Role::Assistant => {
                messages.push(json!({"role": "assistant", "content": message.content}))
            }
            _ => messages.push(json!({"role": "user", "content": message.content})),
        }
    }

    let mut body = json!({
        "model": model,
        "messages": messages,
        // Required by this API, unlike OpenAI's where it is optional.
        "max_tokens": request.max_output.max(1),
    });
    if !system.is_empty() {
        body["system"] = json!(system.join("\n\n"));
    }
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
/// Content is a list of blocks, so prose and tool calls arrive together and
/// both have to be collected. A reader expecting one or the other loses
/// whichever it did not look for.
pub fn decode_response(raw: &str) -> Result<CompletionResponse> {
    let value: Value = serde_json::from_str(raw).map_err(|error| {
        Error::Other(format!("the endpoint returned something that is not JSON: {error}"))
    })?;

    if let Some(message) = value.pointer("/error/message").and_then(Value::as_str) {
        return Err(Error::Other(format!("the endpoint refused: {message}")));
    }

    let blocks = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Other("the endpoint returned a message with no content".into()))?;

    let mut content = String::new();
    let mut reasoning = String::new();
    let mut calls = Vec::new();

    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    content.push_str(text);
                }
            }
            // Extended thinking, where the model is asked for it.
            Some("thinking") => {
                if let Some(text) = block.get("thinking").and_then(Value::as_str) {
                    reasoning.push_str(text);
                }
            }
            Some("tool_use") => {
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| Error::Other("a tool_use block names no tool".into()))?;
                // `input` is an object here, never a string of JSON — that is
                // the OpenAI shape's problem, not this one.
                let arguments = block.get("input").cloned().unwrap_or_else(|| json!({}));
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("call-{}", calls.len()));
                calls.push(IdentifiedCall {
                    id,
                    call: ToolCall { tool: name.to_string(), arguments },
                });
            }
            _ => {}
        }
    }

    let usage = Usage {
        input_tokens: value.pointer("/usage/input_tokens").and_then(Value::as_u64).unwrap_or(0),
        output_tokens: value.pointer("/usage/output_tokens").and_then(Value::as_u64).unwrap_or(0),
    };

    // As with the other backend: never report a finished turn while holding
    // calls, whatever the declared reason says.
    let declared = value.get("stop_reason").and_then(Value::as_str).unwrap_or("end_turn");
    let finish_reason = match declared {
        "max_tokens" => FinishReason::Length,
        "tool_use" => FinishReason::ToolCalls,
        _ if !calls.is_empty() => FinishReason::ToolCalls,
        _ => FinishReason::Stop,
    };

    Ok(CompletionResponse {
        content,
        reasoning: (!reasoning.trim().is_empty()).then_some(reasoning),
        calls,
        usage,
        finish_reason,
    })
}
