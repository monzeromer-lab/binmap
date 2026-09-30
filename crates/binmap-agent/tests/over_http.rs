//! The loop against a real socket (`A1.1`).
//!
//! No model runner is needed: a `TcpListener` in this process speaks enough of
//! OpenAI's chat-completions shape to drive the loop, and it is a *real* HTTP
//! request over a *real* connection. What the recorded-transport tests cannot
//! cover — the client, the headers, the body actually serialising, a non-200
//! being read rather than discarded — this does.
//!
//! The server is deliberately crude. It is not a mock of OpenAI; it is a socket
//! that returns bytes, which is all the transport is entitled to assume.

use binmap_agent::backend::{CompletionRequest, Message, ModelBackend};
use binmap_agent::openai::{OpenAiCompatibleBackend, UreqTransport};
use binmap_agent::provider::provider;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

/// Serve exactly one request, then stop.
///
/// Returns the port and a receiver carrying the request body the server saw, so
/// a test can assert on what was actually put on the wire rather than on what
/// the encoder was asked to produce.
fn serve_once(status: &str, body: &'static str) -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("bound").port();
    let status = status.to_string();
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };

        // Read the headers, then exactly `content-length` bytes of body. A
        // read-to-end would block: the client keeps the connection open.
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut length = 0usize;
        let mut line = String::new();
        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap_or(0);
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
        }
        let mut payload = vec![0u8; length];
        let _ = reader.read_exact(&mut payload);
        let _ = sender.send(String::from_utf8_lossy(&payload).to_string());

        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
             connection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });

    (port, receiver)
}

fn backend_on(port: u16) -> OpenAiCompatibleBackend {
    let spec = provider("local").expect("the local provider");
    let model = spec.default_model().expect("a model");
    OpenAiCompatibleBackend::new(
        spec,
        model,
        Arc::new(UreqTransport::new().with_timeout(Duration::from_secs(10))),
    )
    .with_base_url(format!("http://127.0.0.1:{port}/v1"))
}

const A_REPLY: &str = r#"{
  "choices": [{"message": {"content": "Hypothesis: fmt dominates\nRefuted by: small fmt symbols"},
               "finish_reason": "stop"}],
  "usage": {"prompt_tokens": 31, "completion_tokens": 7}
}"#;

#[test]
fn a_completion_travels_over_a_real_socket() {
    let (port, seen) = serve_once("200 OK", A_REPLY);

    let response = backend_on(port)
        .complete(CompletionRequest::new("qwen3-coder", vec![Message::user("why is this large?")]))
        .expect("the server replied");

    assert!(response.content.contains("fmt dominates"), "{}", response.content);
    assert_eq!(response.usage.input_tokens, 31);
    assert_eq!(response.usage.output_tokens, 7);

    // And what went on the wire is what the encoder built.
    let body = seen.recv_timeout(Duration::from_secs(5)).expect("the server saw a body");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("valid JSON on the wire");
    assert_eq!(parsed["model"], "qwen3-coder");
    assert_eq!(parsed["messages"][0]["role"], "user");
    assert_eq!(parsed["messages"][0]["content"], "why is this large?");
}

#[test]
fn the_registrys_tools_are_declared_on_the_wire() {
    // The one registry, reaching a provider as function declarations. If this
    // drifts, the model is offered tools that do not match what we can run.
    let (port, seen) = serve_once("200 OK", A_REPLY);
    let registry = binmap_agent::Registry::phase_one();
    let tools: Vec<_> = registry.tools().cloned().collect();
    let expected = tools.len();

    let _ = backend_on(port).complete(
        CompletionRequest::new("qwen3-coder", vec![Message::user("go")]).with_tools(tools),
    );

    let body = seen.recv_timeout(Duration::from_secs(5)).expect("a body");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
    let declared = parsed["tools"].as_array().expect("tools were declared");
    assert_eq!(declared.len(), expected, "every registered tool is offered");

    for tool in declared {
        assert_eq!(tool["type"], "function");
        assert!(tool["function"]["name"].is_string());
        assert!(
            tool["function"]["description"].as_str().is_some_and(|d| !d.is_empty()),
            "an external caller has only the description, so it must never be blank"
        );
        assert!(tool["function"]["parameters"].is_object());
    }
}

#[test]
fn a_server_that_returns_html_says_the_body_was_not_json() {
    // A proxy or a wrong port returns HTML, and parsing it as a completion
    // would report "no choices" and send the reader looking at their prompt.
    let (port, _seen) = serve_once("200 OK", "<html><body>hello</body></html>");

    let error = backend_on(port)
        .complete(CompletionRequest::new("qwen3-coder", vec![Message::user("go")]))
        .expect_err("HTML is not a completion");
    assert!(error.to_string().contains("not JSON"), "{error}");
}

#[test]
fn nothing_listening_says_the_runner_has_to_be_running() {
    // The overwhelmingly common first-run failure. It deserves the sentence
    // that fixes it, not a connection-refused trace.
    let port = {
        // Bind and drop, so the port is almost certainly free.
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        listener.local_addr().expect("bound").port()
    };

    let error = backend_on(port)
        .complete(CompletionRequest::new("qwen3-coder", vec![Message::user("go")]))
        .expect_err("there is nothing there");
    let message = error.to_string();
    assert!(message.contains("could not reach"), "{message}");
    assert!(message.contains("has to be running"), "the advice is there: {message}");
}

#[test]
fn the_whole_loop_runs_over_http_and_gates_what_comes_back() {
    // End to end: a real socket, the real transport, the real loop, the real
    // gate. The reply concludes with no tool call, so it cites nothing and the
    // gate must refuse it — which is the correct outcome, and the one that
    // proves the gate is in the path rather than beside it.
    let (port, _seen) = serve_once("200 OK", A_REPLY);

    let registry = binmap_agent::Registry::phase_one();
    let store = binmap_core::evidence::EvidenceStore::new();
    let mut gate = binmap_agent::Gate::new();
    let backend = backend_on(port);

    let mut session = binmap_agent::Session {
        backend: &backend,
        registry: &registry,
        gate: &mut gate,
        store: &store,
        config: binmap_agent::AgentConfig::default(),
        tier: binmap_core::config::TrustTier::Observe,
    };
    let outcome = session.run("why is this binary large?");

    assert_eq!(
        outcome.stop_reason,
        binmap_core::transcript::StopReason::Concluded,
        "the model concluded: {outcome:?}"
    );
    assert!(outcome.spend.usage.input_tokens > 0, "the usage from the wire was counted");

    // A hypothesis was stated, so it is in the transcript.
    assert!(
        outcome.transcript.events().iter().any(|event| matches!(
            event,
            binmap_core::transcript::TranscriptEvent::Hypothesis { .. }
        )),
        "the hypothesis should be recorded"
    );

    // And the conclusion cited nothing, so it is not a finding.
    assert!(outcome.findings.is_empty(), "an ungrounded conclusion is not a finding");
    assert_eq!(outcome.transcript.rejected(), 1, "and the refusal is visible");
}

#[test]
fn a_providers_own_explanation_is_preferred_over_the_status_code() {
    // The failure this fixes: the status was reported and the body discarded,
    // so a runner saying "model not found" by name became a bare "HTTP 404".
    // Ollama with nothing pulled answers exactly this way.
    let (port, _seen) = serve_once(
        "404 Not Found",
        r#"{"error":{"message":"model 'qwen3-coder' not found, try pulling it first"}}"#,
    );

    let error = backend_on(port)
        .complete(CompletionRequest::new("qwen3-coder", vec![Message::user("go")]))
        .expect_err("a 404 is an error");
    let message = error.to_string();
    assert!(message.contains("not found, try pulling it first"), "{message}");
    assert!(message.contains("404"), "the status is still there: {message}");
}

#[test]
fn a_bare_string_error_field_is_also_read() {
    // Some compatible endpoints send `{"error": "..."}` rather than an object.
    let (port, _seen) = serve_once("400 Bad Request", r#"{"error":"context length exceeded"}"#);

    let error = backend_on(port)
        .complete(CompletionRequest::new("qwen3-coder", vec![Message::user("go")]))
        .expect_err("a 400 is an error");
    assert!(error.to_string().contains("context length exceeded"), "{error}");
}

#[test]
fn an_error_status_with_no_usable_body_still_gives_advice() {
    let (port, _seen) = serve_once("500 Internal Server Error", "");

    let error = backend_on(port)
        .complete(CompletionRequest::new("qwen3-coder", vec![Message::user("go")]))
        .expect_err("a 500 is an error");
    let message = error.to_string();
    assert!(message.contains("500"), "{message}");
    assert!(message.contains("model is pulled"), "the fallback advice is there: {message}");
}
