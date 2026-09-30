//! The MCP server (`A2.2`, `A2.4`).
//!
//! The protocol is handled as pure functions, so a whole session runs here
//! without a subprocess. What is being tested is mostly what the server
//! *refuses*: an external agent is the one caller we did not write, and every
//! assumption about how it will behave is one it will eventually violate.

use binmap_agent::mcp::{PROTOCOL_VERSION, Request, Server, ToolOutput, Tools, code, parse, serve};
use binmap_agent::registry::{Registry, ToolCall};
use binmap_core::config::TrustTier;
use serde_json::{Value, json};
use std::sync::Mutex;

/// A tool backend that records what it was asked to run.
#[derive(Default)]
struct Recording {
    calls: Mutex<Vec<ToolCall>>,
}

impl Tools for Recording {
    fn call(&self, call: &ToolCall) -> ToolOutput {
        self.calls.lock().unwrap().push(call.clone());
        ToolOutput {
            text: format!("ran {}", call.tool),
            evidence: Some("ev-0001".into()),
            failed: false,
        }
    }
}

fn request(method: &str, params: Value, id: Option<i64>) -> Request {
    serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": id,
    }))
    .expect("a well-formed request")
}

/// A server that has completed the handshake.
fn ready<'a>(registry: &'a Registry, tools: &'a Recording, tier: TrustTier) -> Server<'a> {
    let mut server = Server::new(registry, tools, tier);
    server.handle(&request("initialize", json!({}), Some(1)));
    server.handle(&request("notifications/initialized", json!({}), None));
    server
}

fn a_real_tool(registry: &Registry) -> String {
    registry.tools().next().expect("Phase 1 registers tools").name.clone()
}

// --- the handshake ----------------------------------------------------------

#[test]
fn initialize_announces_a_pinned_protocol_version() {
    // Echoing back whatever the client proposed is how a server ends up
    // claiming to support a version it has never seen.
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let mut server = Server::new(&registry, &tools, TrustTier::Observe);

    let response = server
        .handle(&request("initialize", json!({"protocolVersion": "1999-01-01"}), Some(1)))
        .expect("a request gets a response");

    assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(response["result"]["serverInfo"]["name"], "binmap");
    assert_eq!(response["id"], 1);
}

#[test]
fn initialize_declares_only_the_capabilities_it_serves() {
    // Declaring resources or prompts we do not serve makes a client offer them
    // to its user and then fail.
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let mut server = Server::new(&registry, &tools, TrustTier::Observe);

    let response = server.handle(&request("initialize", json!({}), Some(1))).unwrap();
    let capabilities = &response["result"]["capabilities"];
    assert!(capabilities.get("tools").is_some());
    assert!(capabilities.get("resources").is_none());
    assert!(capabilities.get("prompts").is_none());
}

#[test]
fn the_instructions_tell_an_external_agent_how_grounding_works() {
    // The description is the whole briefing for an agent we did not write.
    // §5 makes the rejection rate a measurement of us, and an agent never told
    // that claims must cite evidence will fail every one of them.
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let mut server = Server::new(&registry, &tools, TrustTier::Propose);

    let response = server.handle(&request("initialize", json!({}), Some(1))).unwrap();
    let instructions = response["result"]["instructions"].as_str().expect("instructions");

    assert!(instructions.contains("evidence"), "{instructions}");
    assert!(instructions.contains("discarded"), "{instructions}");
    assert!(instructions.contains("Propose"), "the tier is named: {instructions}");
}

#[test]
fn a_notification_gets_no_response() {
    // JSON-RPC forbids answering one, and some clients treat a response to a
    // notification as fatal.
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let mut server = Server::new(&registry, &tools, TrustTier::Observe);

    assert!(server.handle(&request("notifications/initialized", json!({}), None)).is_none());
}

#[test]
fn nothing_can_be_called_before_the_handshake_completes() {
    // A client that skips the handshake has not agreed a protocol version, so
    // anything it is told may be in a shape it cannot read.
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let mut server = Server::new(&registry, &tools, TrustTier::Observe);

    for method in ["tools/list", "tools/call"] {
        let response = server.handle(&request(method, json!({"name": "x"}), Some(9))).unwrap();
        assert_eq!(response["error"]["code"], code::INVALID_REQUEST, "{method}");
        assert!(
            response["error"]["message"].as_str().unwrap().contains("initialise"),
            "{method}: {}",
            response["error"]["message"]
        );
    }
}

// --- the tool list ----------------------------------------------------------

#[test]
fn the_tools_listed_are_the_registrys_own() {
    // §4.1: a tool is described once, and both callers get the same
    // description. Two descriptions of one tool is how they drift.
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let mut server = ready(&registry, &tools, TrustTier::Observe);

    let response = server.handle(&request("tools/list", json!({}), Some(2))).unwrap();
    let listed = response["result"]["tools"].as_array().expect("tools");

    assert_eq!(listed.len(), registry.len());
    for tool in listed {
        assert!(tool["name"].is_string());
        assert!(
            tool["description"].as_str().is_some_and(|d| !d.is_empty()),
            "an external agent has only this string"
        );
        assert!(tool["inputSchema"].is_object(), "MCP names it inputSchema");
    }
}

// --- calling ----------------------------------------------------------------

#[test]
fn a_permitted_tool_runs_and_returns_its_evidence_identifier() {
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let name = a_real_tool(&registry);
    let mut server = ready(&registry, &tools, TrustTier::Observe);

    let response = server
        .handle(&request(
            "tools/call",
            json!({"name": name, "arguments": {"target": "x"}}),
            Some(3),
        ))
        .unwrap();

    assert_eq!(response["result"]["isError"], false);
    assert!(response["result"]["content"][0]["text"].as_str().unwrap().contains(&name));
    // The identifier travels beside the text, so an agent does not have to
    // parse prose to cite correctly.
    assert_eq!(response["result"]["_meta"]["binmap/evidence"], "ev-0001");

    assert_eq!(tools.calls.lock().unwrap().len(), 1, "it actually ran");
}

#[test]
fn a_tool_that_does_not_exist_is_refused_rather_than_invented() {
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let mut server = ready(&registry, &tools, TrustTier::Observe);

    let response = server
        .handle(&request("tools/call", json!({"name": "delete_everything"}), Some(4)))
        .unwrap();

    assert_eq!(response["error"]["code"], code::NOT_PERMITTED);
    assert!(tools.calls.lock().unwrap().is_empty(), "nothing ran");
}

#[test]
fn a_call_with_no_name_is_a_parameter_error_not_a_permission_one() {
    // A client that cannot tell "I asked wrongly" from "I am not allowed"
    // retries the request that will never succeed.
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let mut server = ready(&registry, &tools, TrustTier::Observe);

    let response = server.handle(&request("tools/call", json!({}), Some(5))).unwrap();
    assert_eq!(response["error"]["code"], code::INVALID_PARAMS);
}

#[test]
fn a_tool_needing_a_higher_tier_is_refused_with_the_tier_named() {
    // A2.4. The server never raises its own tier: the prompt belongs in the
    // interface, where a person is, and an agent that could grant itself
    // permission is not operating under one.
    let mut registry = Registry::phase_one();
    registry
        .register(binmap_agent::registry::Tool::writing(
            "apply_configuration",
            "Write a configuration into Cargo.toml.",
            json!({"type": "object"}),
            TrustTier::Tune,
        ))
        .expect("registers");

    let tools = Recording::default();
    let mut server = ready(&registry, &tools, TrustTier::Observe);

    let response = server
        .handle(&request("tools/call", json!({"name": "apply_configuration"}), Some(6)))
        .unwrap();

    assert_eq!(response["error"]["code"], code::NOT_PERMITTED);
    let message = response["error"]["message"].as_str().unwrap();
    assert!(message.contains("Tune"), "the tier it needs is named: {message}");
    assert!(tools.calls.lock().unwrap().is_empty(), "and it did not run");
}

#[test]
fn the_same_tool_runs_once_the_tier_permits_it() {
    // The complement: a server that refused everything would pass the test
    // above and be useless.
    let mut registry = Registry::phase_one();
    registry
        .register(binmap_agent::registry::Tool::writing(
            "apply_configuration",
            "Write a configuration into Cargo.toml.",
            json!({"type": "object"}),
            TrustTier::Tune,
        ))
        .unwrap();

    let tools = Recording::default();
    let mut server = ready(&registry, &tools, TrustTier::Tune);

    let response = server
        .handle(&request("tools/call", json!({"name": "apply_configuration"}), Some(7)))
        .unwrap();
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(tools.calls.lock().unwrap().len(), 1);
}

// --- protocol robustness ----------------------------------------------------

#[test]
fn an_unknown_method_says_what_this_server_does_implement() {
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let mut server = ready(&registry, &tools, TrustTier::Observe);

    let response = server.handle(&request("resources/list", json!({}), Some(8))).unwrap();
    assert_eq!(response["error"]["code"], code::METHOD_NOT_FOUND);
    let message = response["error"]["message"].as_str().unwrap();
    assert!(message.contains("tools/list"), "it names what is available: {message}");
}

#[test]
fn a_malformed_frame_is_answered_rather_than_fatal() {
    // An agent that sends one bad frame should be told, not disconnected.
    let error = parse("{ this is not json").expect_err("not parseable");
    assert_eq!(error["error"]["code"], code::PARSE_ERROR);
    assert_eq!(error["id"], Value::Null, "an unparseable frame has no id to echo");
}

#[test]
fn ping_works_without_touching_anything() {
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let mut server = ready(&registry, &tools, TrustTier::Observe);

    let response = server.handle(&request("ping", json!({}), Some(10))).unwrap();
    assert!(response.get("error").is_none());
    assert!(tools.calls.lock().unwrap().is_empty());
}

// --- a whole session over a pipe -------------------------------------------

#[test]
fn a_whole_session_runs_over_line_delimited_json() {
    // MCP's stdio transport, end to end: handshake, list, call.
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let name = a_real_tool(&registry);
    let mut server = Server::new(&registry, &tools, TrustTier::Observe);

    let session = format!(
        "{}\n{}\n{}\n{}\n",
        json!({"jsonrpc": "2.0", "method": "initialize", "params": {}, "id": 1}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}),
        json!({"jsonrpc": "2.0", "method": "tools/list", "params": {}, "id": 2}),
        json!({"jsonrpc": "2.0", "method": "tools/call",
               "params": {"name": name, "arguments": {}}, "id": 3}),
    );

    let mut input = std::io::BufReader::new(session.as_bytes());
    let mut output = Vec::new();
    serve(&mut server, &mut input, &mut output).expect("the session runs to EOF");

    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("each response is JSON"))
        .collect();

    // Three responses: the notification got none.
    assert_eq!(lines.len(), 3, "{lines:#?}");
    assert_eq!(lines[0]["id"], 1);
    assert_eq!(lines[1]["id"], 2);
    assert_eq!(lines[2]["id"], 3);
    assert!(lines.iter().all(|line| line["jsonrpc"] == "2.0"));
    assert!(lines[2]["result"]["_meta"]["binmap/evidence"].is_string());
}

#[test]
fn a_blank_line_is_skipped_rather_than_answered() {
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let mut server = Server::new(&registry, &tools, TrustTier::Observe);

    let mut input = std::io::BufReader::new("\n\n".as_bytes());
    let mut output = Vec::new();
    serve(&mut server, &mut input, &mut output).unwrap();
    assert!(output.is_empty(), "blank lines are not requests");
}

#[test]
fn a_bad_frame_does_not_end_the_session() {
    let registry = Registry::phase_one();
    let tools = Recording::default();
    let mut server = Server::new(&registry, &tools, TrustTier::Observe);

    let session = format!(
        "not json at all\n{}\n",
        json!({"jsonrpc": "2.0", "method": "initialize", "params": {}, "id": 1})
    );
    let mut input = std::io::BufReader::new(session.as_bytes());
    let mut output = Vec::new();
    serve(&mut server, &mut input, &mut output).unwrap();

    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2, "the bad frame was answered and the good one still ran");
    assert_eq!(lines[0]["error"]["code"], code::PARSE_ERROR);
    assert_eq!(lines[1]["result"]["protocolVersion"], PROTOCOL_VERSION);
}
