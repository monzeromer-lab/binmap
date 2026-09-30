//! One registry, two exposures (`A1.2`, `A2.2`, `AI.5`).
//!
//! Natively to the Mode A loop, and over MCP to external agents. A tool
//! written twice means the abstraction has broken — so a tool is described
//! once, here, and both callers get the same description, the same
//! side-effect declaration and the same tier requirement.
//!
//! **The description is the whole briefing.** In our own loop we write the
//! system prompt and can explain the domain; an external agent arrives knowing
//! nothing and has only this string. A high rejection rate in Mode B means
//! these are unclear, not that the agent is bad — which is why the rejection
//! rate is a measurement of *us*.

use binmap_core::config::TrustTier;
use binmap_core::error::{Error, Result};
use binmap_core::evidence::EvidenceId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What a tool is, in the terms both callers need.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    /// A full sentence, because for an external agent this is all there is.
    pub description: String,
    /// What it accepts, as a JSON Schema. Shared with MCP verbatim.
    pub parameters: serde_json::Value,
    /// Whether running it changes anything outside our own target directory.
    pub side_effects: bool,
    pub required_tier: TrustTier,
}

impl Tool {
    /// A tool that only reads.
    pub fn reading(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            side_effects: false,
            required_tier: TrustTier::Observe,
        }
    }

    /// A tool that changes something, and the tier it needs.
    pub fn writing(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
        required_tier: TrustTier,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            side_effects: true,
            required_tier,
        }
    }
}

/// One invocation a model asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub arguments: serde_json::Value,
}

/// What came back, and the evidence it minted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolOutcome {
    pub output: String,
    /// The identifier a finding may cite for this. Minted by the evidence
    /// store before the output was returned, like everything else.
    pub evidence: EvidenceId,
}

/// Every tool, described once.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    tools: BTreeMap<String, Tool>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// The tools Phase 1 exposes.
    ///
    /// Read-only, every one. Nothing a model can ask for changes anything,
    /// which makes the trust question in Phase 1 entirely about what it is
    /// allowed to *conclude* rather than what it is allowed to *do* — one
    /// problem at a time.
    pub fn phase_one() -> Self {
        let mut registry = Self::new();
        for tool in [
            Tool::reading(
                "list_symbols",
                "List the symbols in the built artifact with their sizes, the section each \
                 lives in, and whether the size was declared by the symbol table or derived \
                 from the next symbol's address. Returns the largest first. Use this to find \
                 out what is actually taking up space before forming any hypothesis about why.",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "How many symbols to return, largest first.",
                            "default": 50
                        },
                        "matching": {
                            "type": "string",
                            "description": "Only symbols whose demangled name contains this."
                        }
                    }
                }),
            ),
            Tool::reading(
                "attribute_size",
                "Attribute the artifact's bytes to the crate each came from, to a category of \
                 cost (formatting machinery, panic strings, unwinding tables, Drop glue, \
                 vtables, static data), and to the generic each was instantiated from. This is \
                 measured from the symbol table; it does not guess.",
                serde_json::json!({ "type": "object", "properties": {} }),
            ),
            Tool::reading(
                "list_instantiations",
                "List the generics that were instantiated more than once, with the aggregate \
                 cost across instantiations and the type arguments seen. Ranked by total cost \
                 rather than by instantiation count, because twelve copies of a small function \
                 matter less than three of a large one.",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "default": 20 }
                    }
                }),
            ),
            Tool::reading(
                "source_for_symbol",
                "Give the source file and line a symbol's code came from, together with any \
                 frames the compiler inlined into it. Requires the artifact to carry debug \
                 info; returns nothing when it does not, which is normal for a release build.",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "symbol": {
                            "type": "string",
                            "description": "The demangled symbol name."
                        }
                    },
                    "required": ["symbol"]
                }),
            ),
            Tool::reading(
                "sweep_results",
                "The configurations measured in this session, with size, runtime and build \
                 time for each, which gates each passed, and which are on the Pareto \
                 frontier. Runtime is only present where the project declares a benchmark.",
                serde_json::json!({ "type": "object", "properties": {} }),
            ),
        ] {
            registry.register(tool).expect("the built-in tools are distinct");
        }
        registry
    }

    /// Register a tool.
    ///
    /// Refuses a duplicate name rather than replacing: two registrations of
    /// one name is the abstraction breaking, not a preference.
    pub fn register(&mut self, tool: Tool) -> Result<()> {
        if self.tools.contains_key(&tool.name) {
            return Err(Error::Other(format!("`{}` is already registered", tool.name)));
        }
        self.tools.insert(tool.name.clone(), tool);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Tool> {
        self.tools.get(name)
    }

    pub fn tools(&self) -> impl Iterator<Item = &Tool> {
        self.tools.values()
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Check a call before it runs — the same check whether the caller is our
    /// own loop or an agent over MCP.
    pub fn authorize(&self, call: &ToolCall, tier: TrustTier) -> Result<&Tool> {
        let tool = self.get(&call.tool).ok_or_else(|| {
            // Naming what does exist, because a model that asked for the wrong
            // tool can correct itself given the list and cannot given "no".
            Error::Other(format!(
                "`{}` is not a tool. Available: {}",
                call.tool,
                self.tools.keys().cloned().collect::<Vec<_>>().join(", ")
            ))
        })?;
        tier.require(tool.required_tier, &tool.name)?;
        Ok(tool)
    }

    /// The registry as MCP describes tools, for the server to serve verbatim.
    ///
    /// Generated rather than hand-written, which is what makes "a tool written
    /// twice means the abstraction has broken" enforceable rather than
    /// aspirational.
    pub fn as_mcp(&self) -> serde_json::Value {
        serde_json::json!({
            "tools": self
                .tools
                .values()
                .map(|tool| serde_json::json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": tool.parameters,
                }))
                .collect::<Vec<_>>()
        })
    }
}
