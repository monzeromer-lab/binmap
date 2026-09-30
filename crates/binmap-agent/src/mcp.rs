//! The MCP server (`A2.2`, `A2.4`, `DESIGN-AI §4`).
//!
//! The registry's second exposure. `§4.1` is that a tool is described once and
//! both callers get the same description — natively to our own loop, and over
//! MCP to an external agent. Two descriptions of one tool is how they drift,
//! and the drift is invisible until an agent misuses a tool because its
//! briefing was the stale copy.
//!
//! **The description is the whole briefing here.** In Mode A we write the
//! system prompt and can explain the domain. An external agent arrives knowing
//! nothing and has only these strings, which is why `§5` makes the rejection
//! rate a measurement of *us*: a high one means the descriptions are unclear,
//! not that the agent is bad.
//!
//! Two things this server will not do, both deliberate:
//!
//! - **It does not raise its own trust tier.** A tool needing `Tune` is
//!   refused at `Observe` with the tier named, rather than prompting. The
//!   prompt belongs in the interface, where a person is, and an agent that
//!   could grant itself permission is not operating under one.
//! - **It does not mint evidence.** Tool results carry the identifiers the
//!   evidence store issued, and the airlock later asks the store whether it
//!   issued them. An agent that could invent an identifier could ground any
//!   claim it liked.

use crate::registry::{Registry, ToolCall};
use binmap_core::config::TrustTier;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The protocol version this server speaks.
///
/// Pinned rather than echoed back. Agreeing to whatever a client proposes is
/// how a server ends up claiming to support a version it has never seen.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// A JSON-RPC request, as much of it as matters.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Request {
    pub method: String,
    #[serde(default)]
    pub params: Value,
    /// Absent for a notification, which takes no response.
    #[serde(default)]
    pub id: Option<Value>,
}

/// JSON-RPC's own error codes, plus the one we add.
pub mod code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    /// Not a protocol error: the request was well-formed and the tool was
    /// refused. Distinguished so a client can tell "I asked wrongly" from "I
    /// am not allowed".
    pub const NOT_PERMITTED: i64 = -32000;
}

/// What a tool call returned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolOutput {
    pub text: String,
    /// The identifier a claim may cite for this.
    ///
    /// Minted by the evidence store before the output was returned, like
    /// everything else. An agent cannot invent one that survives the airlock.
    pub evidence: Option<String>,
    pub failed: bool,
}

/// What the server is willing to do on the caller's behalf.
///
/// Injected rather than reached for, so the protocol layer can be tested
/// without an engine and so the server cannot widen its own reach.
pub trait Tools {
    /// Run a tool that has already been authorised.
    fn call(&self, call: &ToolCall) -> ToolOutput;
}

/// One MCP session.
pub struct Server<'a> {
    pub registry: &'a Registry,
    pub tools: &'a dyn Tools,
    /// The tier the *user* set. The server never raises it.
    pub tier: TrustTier,
    /// Set by `initialize`, so a call before it is refused.
    initialised: bool,
}

impl<'a> Server<'a> {
    pub fn new(registry: &'a Registry, tools: &'a dyn Tools, tier: TrustTier) -> Self {
        Self { registry, tools, tier, initialised: false }
    }

    /// Handle one request.
    ///
    /// `None` for a notification, which by JSON-RPC takes no response — and
    /// answering one is a protocol violation that some clients treat as fatal.
    pub fn handle(&mut self, request: &Request) -> Option<Value> {
        let id = request.id.clone();

        // A notification. `notifications/initialized` is the one that matters:
        // it confirms the handshake, and nothing may be called before it.
        if id.is_none() {
            if request.method == "notifications/initialized" {
                self.initialised = true;
            }
            return None;
        }
        let id = id.expect("checked");

        let result = match request.method.as_str() {
            "initialize" => Ok(self.initialize()),
            "tools/list" => self.list(),
            "tools/call" => self.call(&request.params),
            // `ping` exists so a client can check the connection without
            // touching anything.
            "ping" => Ok(json!({})),
            other => Err((
                code::METHOD_NOT_FOUND,
                format!(
                    "this server implements initialize, tools/list, tools/call and ping. It was \
                     asked for `{other}`."
                ),
            )),
        };

        Some(match result {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err((code, message)) => {
                json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
            }
        })
    }

    fn initialize(&self) -> Value {
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            // Tools only. Declaring resources or prompts we do not serve would
            // make a client offer them to its user and then fail.
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {"name": "binmap", "version": env!("CARGO_PKG_VERSION")},
            // Not part of the spec, and deliberately included: an agent that
            // knows the tier up front can avoid asking for what it cannot have.
            "instructions": format!(
                "Binmap exposes read-only analysis of compiled artifacts. Every tool result \
                 carries an `evidence` identifier; a claim you make must cite the identifiers of \
                 the results that support it, and a claim citing an identifier this session did \
                 not issue is discarded. The current trust tier is `{}`, which means: {}",
                self.tier.label(),
                self.tier.permits()
            ),
        })
    }

    fn list(&self) -> std::result::Result<Value, (i64, String)> {
        if !self.initialised {
            return Err((
                code::INVALID_REQUEST,
                "this session has not been initialised. Send `initialize`, then the \
                 `notifications/initialized` notification, before listing tools."
                    .into(),
            ));
        }
        Ok(self.registry.as_mcp())
    }

    fn call(&self, params: &Value) -> std::result::Result<Value, (i64, String)> {
        if !self.initialised {
            return Err((code::INVALID_REQUEST, "this session has not been initialised.".into()));
        }

        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or((code::INVALID_PARAMS, "a tools/call needs a `name`".to_string()))?;
        let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

        let call = ToolCall { tool: name.to_string(), arguments };

        // Authorisation is the registry's, not the server's, so Mode A and
        // Mode B cannot disagree about what a tier permits.
        let tool = self.registry.authorize(&call, self.tier).map_err(|error| {
            // A refused tool is not a protocol error: the request was
            // well-formed. A client that cannot tell the two apart retries the
            // request that will never succeed.
            (code::NOT_PERMITTED, error.to_string())
        })?;
        let _ = tool;

        let output = self.tools.call(&call);

        Ok(json!({
            // MCP's content shape. The evidence identifier travels beside it
            // rather than inside the text, so an agent does not have to parse
            // prose to cite correctly.
            "content": [{"type": "text", "text": output.text}],
            "isError": output.failed,
            "_meta": {"binmap/evidence": output.evidence},
        }))
    }
}

/// Parse one line of JSON-RPC.
///
/// Separated from `handle` so a malformed line produces a proper error
/// response rather than killing the session: an agent that sends one bad frame
/// should get told, not disconnected.
pub fn parse(line: &str) -> std::result::Result<Request, Value> {
    serde_json::from_str::<Request>(line).map_err(|error| {
        json!({
            "jsonrpc": "2.0",
            "id": Value::Null,
            "error": {
                "code": code::PARSE_ERROR,
                "message": format!("that is not a JSON-RPC request: {error}")
            }
        })
    })
}

/// Run a session over a reader and a writer.
///
/// Line-delimited JSON, which is what MCP's stdio transport is.
pub fn serve(
    server: &mut Server<'_>,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(());
        }
        if line.trim().is_empty() {
            continue;
        }

        let response = match parse(&line) {
            Ok(request) => server.handle(&request),
            Err(error) => Some(error),
        };

        if let Some(response) = response {
            writeln!(output, "{response}")?;
            // Flushed per response: a client waiting on an answer that is
            // sitting in our buffer looks like a hang.
            output.flush()?;
        }
    }
}
