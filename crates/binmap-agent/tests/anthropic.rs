//! Claude's messages API (`A1.4`, `DESIGN-AI §6.1`).
//!
//! Every test here covers a way this shape differs *structurally* from
//! OpenAI's, which is why it is a second backend rather than a quirks entry on
//! the first. Each difference has a failure mode that is quiet rather than
//! loud: a system prompt sent as a message is accepted and ignored, and a
//! response read as OpenAI's shape simply finds no tool calls.

use binmap_agent::anthropic::{API_VERSION, decode_response, encode_request, encode_tool};
use binmap_agent::backend::{CompletionRequest, FinishReason, Message, ModelBackend, Role};
use binmap_agent::openai::HttpTransport;
use binmap_agent::provider::provider;
use binmap_agent::registry::Tool;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// One request, as the transport saw it: the body and the headers.
type Seen = (Value, Vec<(String, String)>);

#[derive(Debug)]
struct Scripted {
    reply: String,
    seen: Mutex<Vec<Seen>>,
}

impl Scripted {
    fn new(reply: impl Into<String>) -> Arc<Self> {
        Arc::new(Self { reply: reply.into(), seen: Mutex::new(Vec::new()) })
    }
    fn last(&self) -> Seen {
        self.seen.lock().unwrap().last().cloned().expect("something was posted")
    }
}

impl HttpTransport for Scripted {
    fn post_json(
        &self,
        _url: &str,
        headers: &[(&str, String)],
        body: &Value,
    ) -> binmap_core::Result<String> {
        self.seen.lock().unwrap().push((
            body.clone(),
            headers.iter().map(|(n, v)| ((*n).to_string(), v.clone())).collect(),
        ));
        Ok(self.reply.clone())
    }
}

fn backend(transport: Arc<Scripted>) -> binmap_agent::anthropic::AnthropicBackend {
    let spec = provider("anthropic").expect("Claude is in the table");
    let model = spec.default_model().expect("a model");
    binmap_agent::anthropic::AnthropicBackend::new(spec, model, transport)
}

const A_REPLY: &str = r#"{
  "content": [{"type": "text", "text": "done"}],
  "stop_reason": "end_turn",
  "usage": {"input_tokens": 11, "output_tokens": 3}
}"#;

#[test]
fn the_system_prompt_is_a_top_level_field_not_a_message() {
    // Left as a message it is accepted and *ignored*, so the symptom is a
    // model that will not follow instructions rather than an error anyone can
    // act on.
    let request = CompletionRequest::new(
        "claude",
        vec![Message::system("cite your evidence"), Message::user("why is this large?")],
    );
    let body = encode_request(&request, "claude-sonnet-5-5");

    assert_eq!(body["system"], "cite your evidence");
    let messages = body["messages"].as_array().expect("messages");
    assert_eq!(messages.len(), 1, "the system turn is not among them");
    assert_eq!(messages[0]["role"], "user");
}

#[test]
fn several_system_turns_are_joined_rather_than_one_being_dropped() {
    let request = CompletionRequest::new(
        "claude",
        vec![Message::system("first"), Message::system("second"), Message::user("go")],
    );
    let body = encode_request(&request, "m");
    let system = body["system"].as_str().expect("a system field");
    assert!(system.contains("first") && system.contains("second"), "{system}");
}

#[test]
fn a_tool_result_is_a_block_inside_a_user_turn() {
    // There is no `tool` role in this API. Sending one is rejected, and
    // sending it as a plain user message loses the link to the call.
    let request = CompletionRequest::new(
        "claude",
        vec![Message::user("go"), Message::tool("toolu_1", "42 bytes")],
    );
    let body = encode_request(&request, "m");
    let messages = body["messages"].as_array().unwrap();

    let last = messages.last().expect("a turn");
    assert_eq!(last["role"], "user");
    let block = &last["content"][0];
    assert_eq!(block["type"], "tool_result");
    assert_eq!(block["tool_use_id"], "toolu_1");
    assert_eq!(block["content"], "42 bytes");
}

#[test]
fn consecutive_tool_results_share_one_user_turn() {
    // The API rejects two user turns in a row, so a step that ran several
    // tools would fail on its second result.
    let request = CompletionRequest::new(
        "claude",
        vec![
            Message::user("go"),
            Message::tool("toolu_1", "first"),
            Message::tool("toolu_2", "second"),
        ],
    );
    let body = encode_request(&request, "m");
    let messages = body["messages"].as_array().unwrap();

    let roles: Vec<&str> = messages.iter().map(|m| m["role"].as_str().unwrap()).collect();
    assert_eq!(roles, vec!["user", "user"], "one turn for the prompt, one for both results");
    assert_eq!(
        messages[1]["content"].as_array().expect("blocks").len(),
        2,
        "both results are blocks in the same turn"
    );
}

#[test]
fn a_tools_schema_sits_at_the_top_level_as_input_schema() {
    // OpenAI nests it under `function`. Sending that shape here is rejected.
    let schema = json!({"type": "object", "properties": {"path": {"type": "string"}}});
    let tool = Tool::reading("read_symbols", "Read the symbol table.", schema.clone());

    let encoded = encode_tool(&tool);
    assert_eq!(encoded["name"], "read_symbols");
    assert_eq!(encoded["description"], "Read the symbol table.");
    assert_eq!(encoded["input_schema"], schema, "the schema passes through verbatim");
    assert!(encoded.get("function").is_none(), "there is no function envelope here");
}

#[test]
fn max_tokens_is_always_sent_because_this_api_requires_it() {
    // Optional for OpenAI, required here. Omitting it is a 400 on every call.
    let request = CompletionRequest::new("claude", vec![Message::user("go")]);
    let body = encode_request(&request, "m");
    assert!(body["max_tokens"].as_u64().unwrap_or(0) > 0, "{body}");
}

#[test]
fn authentication_uses_x_api_key_and_a_version_header() {
    // A bearer token is silently unauthenticated here.
    // SAFETY: single-threaded test, and the value is a test credential for an
    // endpoint that is never reached.
    unsafe { std::env::set_var("ANTHROPIC_API_KEY", "test-key-not-real") };
    let transport = Scripted::new(A_REPLY);

    backend(transport.clone())
        .complete(CompletionRequest::new("m", vec![Message::user("hi")]))
        .expect("the scripted transport replies");

    let (_, headers) = transport.last();
    let header =
        |name: &str| headers.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone());
    assert_eq!(header("x-api-key").as_deref(), Some("test-key-not-real"));
    assert_eq!(header("anthropic-version").as_deref(), Some(API_VERSION));
    assert!(header("authorization").is_none(), "a bearer token is not how this authenticates");

    unsafe { std::env::remove_var("ANTHROPIC_API_KEY") };
}

// --- decoding: content is a list of blocks ---------------------------------

#[test]
fn prose_and_a_tool_call_arrive_together_and_both_are_kept() {
    // A reader expecting one or the other loses whichever it did not look
    // for — and losing the call means the loop concludes early.
    let raw = json!({
        "content": [
            {"type": "text", "text": "Hypothesis: fmt dominates"},
            {"type": "tool_use", "id": "toolu_9", "name": "read_symbols", "input": {"path": "a"}}
        ],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 5, "output_tokens": 7}
    })
    .to_string();

    let response = decode_response(&raw).expect("a valid message");
    assert!(response.content.contains("fmt dominates"));
    assert_eq!(response.calls.len(), 1);
    assert_eq!(response.calls[0].id, "toolu_9");
    assert_eq!(response.calls[0].call.arguments["path"], "a");
    assert_eq!(response.finish_reason, FinishReason::ToolCalls);
}

#[test]
fn tool_input_is_an_object_rather_than_a_string_of_json() {
    // The OpenAI shape's problem, and not this one. Parsing it as a string
    // would fail on every call.
    let raw = json!({
        "content": [{"type": "tool_use", "id": "t", "name": "x", "input": {"n": 1}}],
        "stop_reason": "tool_use"
    })
    .to_string();
    let response = decode_response(&raw).unwrap();
    assert_eq!(response.calls[0].call.arguments["n"], 1);
}

#[test]
fn extended_thinking_is_read_as_reasoning_rather_than_as_a_claim() {
    let raw = json!({
        "content": [
            {"type": "thinking", "thinking": "weighing the options"},
            {"type": "text", "text": "done"}
        ],
        "stop_reason": "end_turn"
    })
    .to_string();
    let response = decode_response(&raw).unwrap();
    assert_eq!(response.reasoning.as_deref(), Some("weighing the options"));
    assert_eq!(response.content, "done", "thinking is not part of what it said");
}

#[test]
fn a_response_holding_calls_is_never_reported_as_finished() {
    let raw = json!({
        "content": [{"type": "tool_use", "id": "t", "name": "x", "input": {}}],
        "stop_reason": "end_turn"
    })
    .to_string();
    assert_eq!(decode_response(&raw).unwrap().finish_reason, FinishReason::ToolCalls);
}

#[test]
fn hitting_the_output_limit_is_reported_as_truncation() {
    let raw = json!({
        "content": [{"type": "text", "text": "the largest driver is"}],
        "stop_reason": "max_tokens"
    })
    .to_string();
    assert_eq!(decode_response(&raw).unwrap().finish_reason, FinishReason::Length);
}

#[test]
fn usage_uses_this_apis_own_field_names() {
    // `input_tokens`, not `prompt_tokens`. Reading the wrong names gives a
    // cost meter that always says zero.
    let response = decode_response(A_REPLY).unwrap();
    assert_eq!(response.usage.input_tokens, 11);
    assert_eq!(response.usage.output_tokens, 3);
}

#[test]
fn an_error_body_is_reported_as_the_refusal_it_is() {
    let raw = json!({"type": "error", "error": {"message": "invalid x-api-key"}}).to_string();
    let error = decode_response(&raw).expect_err("an error body is an error");
    assert!(error.to_string().contains("invalid x-api-key"), "{error}");
}

#[test]
fn a_message_with_no_content_is_an_error_rather_than_an_empty_answer() {
    let error = decode_response(&json!({"stop_reason": "end_turn"}).to_string())
        .expect_err("no content is a broken response");
    assert!(error.to_string().contains("no content"), "{error}");
}

#[test]
fn an_unknown_block_type_is_skipped_rather_than_failing_the_whole_response() {
    // A block type added after this was written must not break a session.
    let raw = json!({
        "content": [
            {"type": "some_future_block", "whatever": 1},
            {"type": "text", "text": "still here"}
        ],
        "stop_reason": "end_turn"
    })
    .to_string();
    assert_eq!(decode_response(&raw).unwrap().content, "still here");
}

#[test]
fn the_backend_is_reachable_through_the_provider_table() {
    // The whole point of the table: adding Claude is a row plus a wire shape,
    // and `backend_for` is the only place that knows there are two shapes.
    let spec = provider("anthropic").unwrap();
    let backend = binmap_agent::backend::backend_for(
        spec,
        "claude-sonnet-5-5",
        Arc::new(binmap_agent::UnavailableTransport),
    )
    .map(|backend| backend.id().to_string());

    assert_eq!(backend.ok().as_deref(), Some("anthropic/claude-sonnet-5-5"));
}

#[test]
fn a_system_prompt_with_no_user_turn_still_travels() {
    let request = CompletionRequest::new("claude", vec![Message::system("the briefing")]);
    let body = encode_request(&request, "m");
    assert_eq!(body["system"], "the briefing");
    assert!(body["messages"].as_array().unwrap().is_empty());
    // And nothing claims the system prompt became a user turn.
    assert!(!matches!(request.messages[0].role, Role::User));
}
