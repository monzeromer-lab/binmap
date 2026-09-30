//! The OpenAI-compatible wire format, tested without a network.
//!
//! Everything that can be wrong about talking to one of these endpoints lives
//! in the translation, and the translation is pure. That is the whole reason
//! `HttpTransport` is a trait: this file covers the format on a machine with
//! no model runner installed.

use binmap_agent::backend::{CompletionRequest, FinishReason, Message, ModelBackend, Role};
use binmap_agent::openai::{
    HttpTransport, OpenAiCompatibleBackend, UnavailableTransport, apply_quirks, decode_response,
    encode_request, encode_tool,
};
use binmap_agent::provider::{Quirks, provider};
use binmap_agent::registry::Tool;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// A transport that returns what it was told to, and remembers what it was
/// asked.
#[derive(Debug)]
struct Scripted {
    reply: String,
    seen: Mutex<Vec<Value>>,
}

impl Scripted {
    fn new(reply: impl Into<String>) -> Arc<Self> {
        Arc::new(Self { reply: reply.into(), seen: Mutex::new(Vec::new()) })
    }

    fn last_body(&self) -> Value {
        self.seen.lock().unwrap().last().cloned().expect("something was posted")
    }
}

impl HttpTransport for Scripted {
    fn post_json(
        &self,
        _url: &str,
        _key: Option<&str>,
        body: &Value,
    ) -> binmap_core::Result<String> {
        self.seen.lock().unwrap().push(body.clone());
        Ok(self.reply.clone())
    }
}

fn completion(content: &str) -> String {
    json!({
        "choices": [{"message": {"content": content}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5}
    })
    .to_string()
}

#[test]
fn a_tool_is_rendered_from_the_registry_schema_unchanged() {
    // §4.1: the tool is described once. Two renderings is how a description
    // drifts from the one an MCP client sees.
    let schema = json!({"type": "object", "properties": {"symbol": {"type": "string"}}});
    let tool = Tool::reading("read_symbols", "Read the symbol table.", schema.clone());

    let encoded = encode_tool(&tool);
    assert_eq!(encoded["type"], "function");
    assert_eq!(encoded["function"]["name"], "read_symbols");
    assert_eq!(encoded["function"]["description"], "Read the symbol table.");
    assert_eq!(encoded["function"]["parameters"], schema, "the schema passes through verbatim");
}

#[test]
fn a_tool_result_carries_the_id_of_the_call_it_answers() {
    // Without it the model sees an unattached result and cannot tell which of
    // several calls it belongs to.
    let request =
        CompletionRequest::new("m", vec![Message::user("go"), Message::tool("call_7", "42 bytes")]);
    let body = encode_request(&request, "m");
    let messages = body["messages"].as_array().unwrap();

    assert_eq!(messages[1]["role"], "tool");
    assert_eq!(messages[1]["tool_call_id"], "call_7");
    assert!(
        messages[0].get("tool_call_id").is_none(),
        "a user message has no call to answer, so the field is absent rather than null"
    );
}

#[test]
fn tool_arguments_arrive_as_a_string_of_json_and_are_parsed() {
    // The normal case for every one of these endpoints.
    let raw = json!({
        "choices": [{
            "message": {
                "content": "",
                "tool_calls": [{
                    "id": "call_1",
                    "function": {"name": "read_symbols", "arguments": "{\"symbol\":\"main\"}"}
                }]
            },
            "finish_reason": "tool_calls"
        }]
    })
    .to_string();

    let response = decode_response(&raw).expect("a well-formed tool call");
    assert_eq!(response.calls.len(), 1);
    assert_eq!(response.calls[0].id, "call_1");
    assert_eq!(response.calls[0].call.tool, "read_symbols");
    assert_eq!(response.calls[0].call.arguments["symbol"], "main");
    assert_eq!(response.finish_reason, FinishReason::ToolCalls);
}

#[test]
fn tool_arguments_sent_as_a_bare_object_are_also_accepted() {
    // Some local runners do this. Refusing would make the weakest provider —
    // the one §7.3 says to author against — the one that does not work.
    let raw = json!({
        "choices": [{
            "message": {
                "tool_calls": [{
                    "id": "c",
                    "function": {"name": "t", "arguments": {"symbol": "main"}}
                }]
            }
        }]
    })
    .to_string();

    let response = decode_response(&raw).expect("an object is as good as a string of one");
    assert_eq!(response.calls[0].call.arguments["symbol"], "main");
}

#[test]
fn a_tool_that_takes_no_arguments_is_not_an_error() {
    for arguments in [json!(""), json!(null), json!("{}")] {
        let raw = json!({
            "choices": [{
                "message": {"tool_calls": [{"id": "c", "function": {"name": "t", "arguments": arguments}}]}
            }]
        })
        .to_string();
        let response = decode_response(&raw).unwrap_or_else(|error| {
            panic!("arguments {arguments} should decode to an empty object, got: {error}")
        });
        assert_eq!(response.calls[0].call.arguments, json!({}));
    }
}

#[test]
fn malformed_tool_arguments_say_what_arrived() {
    // Quirks::malformed_tool_arguments in the wild. The loop retries once with
    // this message fed back, so it has to be usable as feedback: naming the
    // problem is not enough without showing what was received.
    let raw = json!({
        "choices": [{
            "message": {
                "tool_calls": [{"id": "c", "function": {"name": "t", "arguments": "{symbol: main"}}]
            }
        }]
    })
    .to_string();

    let error = decode_response(&raw).expect_err("invalid JSON must not be silently dropped");
    let message = error.to_string();
    assert!(message.contains("not valid JSON"), "{message}");
    assert!(message.contains("{symbol: main"), "the received text is shown back: {message}");
}

#[test]
fn a_response_holding_calls_is_never_reported_as_finished() {
    // Some endpoints say "stop" and attach tool calls anyway. A loop that
    // believed the declared reason would drop the calls and conclude early —
    // silently, with a wrong answer.
    let raw = json!({
        "choices": [{
            "message": {
                "content": "let me look",
                "tool_calls": [{"id": "c", "function": {"name": "t", "arguments": "{}"}}]
            },
            "finish_reason": "stop"
        }]
    })
    .to_string();

    let response = decode_response(&raw).unwrap();
    assert_eq!(
        response.finish_reason,
        FinishReason::ToolCalls,
        "a response with calls wants to call them, whatever it claims"
    );
}

#[test]
fn a_truncated_response_is_reported_as_truncated() {
    // Parsing a half-sentence as a complete answer is how a conclusion gets
    // built from something the model never finished saying.
    let raw = json!({
        "choices": [{"message": {"content": "the largest driver is"}, "finish_reason": "length"}]
    })
    .to_string();
    assert_eq!(decode_response(&raw).unwrap().finish_reason, FinishReason::Length);
}

#[test]
fn an_error_body_is_reported_as_the_refusal_it_is() {
    // The common case when a key is wrong or a model name is misspelled.
    // "No choices returned" would send the reader looking in the wrong place.
    let raw = json!({"error": {"message": "invalid api key"}}).to_string();
    let error = decode_response(&raw).expect_err("an error body is an error");
    assert!(error.to_string().contains("invalid api key"), "{error}");
}

#[test]
fn a_body_that_is_not_json_says_so() {
    let error = decode_response("<html>502 Bad Gateway</html>").expect_err("not JSON");
    assert!(error.to_string().contains("not JSON"), "{error}");
}

#[test]
fn an_empty_completion_is_an_error_rather_than_an_empty_answer() {
    // A backend returning nothing successfully would let the loop run to its
    // step budget producing nothing, and call that a result.
    let error = decode_response(&json!({"choices": []}).to_string())
        .expect_err("no choices is a broken endpoint, not a model with nothing to say");
    assert!(error.to_string().contains("no choices"), "{error}");
}

#[test]
fn reasoning_content_is_read_from_either_field_name() {
    for field in ["reasoning_content", "reasoning"] {
        let raw = json!({
            "choices": [{"message": {"content": "done", field: "thinking hard"}}]
        })
        .to_string();
        let response = decode_response(&raw).unwrap();
        assert_eq!(response.reasoning.as_deref(), Some("thinking hard"), "field {field}");
    }
}

#[test]
fn blank_reasoning_is_absent_rather_than_empty() {
    // An empty thought bubble in the transcript is worse than no bubble.
    let raw =
        json!({"choices": [{"message": {"content": "done", "reasoning": "   "}}]}).to_string();
    assert_eq!(decode_response(&raw).unwrap().reasoning, None);
}

#[test]
fn a_provider_that_refuses_a_system_prompt_gets_it_folded_into_the_first_user_turn() {
    // Quirks::no_system_prompt. Dropping the briefing would look like a model
    // that will not follow instructions.
    let request = CompletionRequest::new(
        "m",
        vec![Message::system("cite your evidence"), Message::user("why is this large?")],
    );
    let quirked = apply_quirks(request, &Quirks { no_system_prompt: true, ..Quirks::NONE });

    assert!(
        quirked.messages.iter().all(|message| message.role != Role::System),
        "no system role survives"
    );
    assert_eq!(quirked.messages[0].role, Role::User);
    assert!(quirked.messages[0].content.contains("cite your evidence"), "the briefing survives");
    assert!(quirked.messages[0].content.contains("why is this large?"), "so does the question");
}

#[test]
fn a_system_prompt_with_no_user_turn_to_fold_into_becomes_one() {
    let request = CompletionRequest::new("m", vec![Message::system("the briefing")]);
    let quirked = apply_quirks(request, &Quirks { no_system_prompt: true, ..Quirks::NONE });
    assert_eq!(quirked.messages.len(), 1);
    assert_eq!(quirked.messages[0].role, Role::User);
    assert_eq!(quirked.messages[0].content, "the briefing");
}

#[test]
fn a_provider_with_a_fixed_temperature_is_not_sent_one() {
    let mut request = CompletionRequest::new("m", vec![Message::user("go")]);
    request.temperature = Some(0.7);
    let quirked = apply_quirks(request, &Quirks { fixed_temperature: true, ..Quirks::NONE });
    assert_eq!(quirked.temperature, None);

    let body = encode_request(&quirked, "m");
    assert!(body.get("temperature").is_none(), "and the field is absent from the body");
}

#[test]
fn the_backend_posts_to_the_chat_completions_path_of_its_base_url() {
    let spec = provider("local").expect("the local provider is in the table");
    let model = spec.default_model().unwrap();
    let transport = Scripted::new(completion("hello"));

    let backend = OpenAiCompatibleBackend::new(spec, model, transport.clone())
        .with_base_url("http://127.0.0.1:9999/v1/");
    let response = backend
        .complete(CompletionRequest::new(model.id, vec![Message::user("hi")]))
        .expect("the scripted transport replies");

    assert_eq!(response.content, "hello");
    assert_eq!(response.usage.input_tokens, 10);
    assert_eq!(response.usage.output_tokens, 5);
    // The trailing slash must not produce a double slash in the path.
    assert_eq!(transport.last_body()["model"], model.id);
}

#[test]
fn no_transport_configured_is_a_sentence_rather_than_a_hang() {
    let spec = provider("local").unwrap();
    let model = spec.default_model().unwrap();
    let backend = OpenAiCompatibleBackend::new(spec, model, Arc::new(UnavailableTransport));

    let error = backend
        .complete(CompletionRequest::new(model.id, vec![Message::user("hi")]))
        .expect_err("there is nothing to talk to");
    assert!(error.to_string().contains("no HTTP transport"), "{error}");
}
