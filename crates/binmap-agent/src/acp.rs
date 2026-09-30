//! Mode B: driving an external agent over ACP (`A2.1`, `A2.4`, `DESIGN-AI §2`).
//!
//! ACP is the LSP analogue for agents — JSON-RPC over stdio to a subprocess —
//! and `§2` explains why supporting it is worth one protocol implementation:
//! it is not two agents, it is the whole ecosystem.
//!
//! The load-bearing step is `session/new`, because the client supplies **MCP
//! server configurations** when creating a session. That is how an external
//! agent receives our entire analysis toolset without knowing anything about
//! DWARF, and it is what makes Mode B a real mode rather than a degraded chat
//! window. The server it is handed is the one in `mcp.rs`.
//!
//! Two responsibilities here are ours alone and are where the danger is:
//!
//! - **Permissions.** `session/request_permission` maps onto the trust ladder
//!   (`§9.3`), and the mapping is a table rather than a judgement call.
//! - **Filesystem scope.** An agent asks the *client* to read files. `§9.3` is
//!   explicit: an agent asking for `~/.ssh/id_rsa` "should get a refusal and a
//!   visible warning, not a permission prompt". A prompt would make it the
//!   user's mistake to click through.
//!
//! The protocol types come from `agent-client-protocol-schema`, the official
//! crate's type half. The transport is ours because the engine is blocking
//! everywhere else; the types are the part that drifts with the protocol, and
//! those are not hand-rolled.

use binmap_core::config::TrustTier;
use binmap_core::transcript::{Origin, TranscriptEvent};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// What an agent asked to be allowed to do.
///
/// Derived from the request's tool kind rather than its prose: an agent
/// describing a write as "just updating a file" must not thereby get a
/// read-only decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Requested {
    /// Reading a file, listing a directory, running an analysis.
    Read,
    /// Writing build configuration — `Cargo.toml`, a bundler flag.
    BuildConfiguration,
    /// Editing source.
    SourceEdit,
    /// Running a command.
    Execute,
    /// Something the protocol named that we do not recognise.
    Unknown,
}

/// What we do about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    /// Allowed without asking.
    Allow,
    /// Ask the user. Carries the sentence the dialog shows.
    Ask { because: String },
    /// Refused without asking, and why.
    ///
    /// Distinct from `Ask` on purpose. Some refusals must not be presented as
    /// a choice — an agent reaching outside the project is not a decision to
    /// delegate to someone in a hurry.
    Refuse { because: String },
}

impl Decision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Decision::Allow)
    }

    pub fn needs_a_person(&self) -> bool {
        matches!(self, Decision::Ask { .. })
    }

    pub fn describe(&self) -> String {
        match self {
            Decision::Allow => "allowed".into(),
            Decision::Ask { because } => format!("asking: {because}"),
            Decision::Refuse { because } => format!("refused: {because}"),
        }
    }
}

/// `§9.3`'s table, as a table.
///
/// > | Observe | Auto-deny anything beyond read-only |
/// > | Propose | Auto-allow read-only; prompt for anything that writes |
/// > | Tune | Auto-allow build-config changes; prompt for source edits |
/// > | Autonomous | Auto-allow within the workspace; still gated by the verifier |
///
/// The last row is the one to read carefully. **Permission gets an agent to
/// the verifier; it does not get it past one.** An agent allowed to edit
/// source still cannot land a change that fails to build, breaks tests or
/// regresses a benchmark.
pub fn decide(tier: TrustTier, requested: Requested) -> Decision {
    use Requested::*;
    use TrustTier::*;

    match (tier, requested) {
        // Reading is always fine: every tier's floor is Observe, and Observe
        // measures and explains.
        (_, Read) => Decision::Allow,

        (Observe, _) => Decision::Refuse {
            because: "the trust tier is Observe, which measures and explains and writes nothing. \
                      Raise it in the Agent panel if you want this."
                .into(),
        },

        (Propose, _) => Decision::Ask {
            because: "the trust tier is Propose, which generates changes and never applies them. \
                      This would write."
                .into(),
        },

        (Tune, BuildConfiguration) => Decision::Allow,
        (Tune, SourceEdit) => Decision::Ask {
            because: "the trust tier is Tune, which writes build configuration only. This edits \
                      source."
                .into(),
        },
        (Tune, Execute) => Decision::Ask {
            because: "running a command is not a build-configuration change, so Tune asks.".into(),
        },

        (Autonomous, SourceEdit | BuildConfiguration | Execute) => Decision::Allow,

        // An unrecognised kind is never auto-allowed, whatever the tier. A
        // protocol extension we have not seen is exactly the thing not to
        // grant blind.
        (_, Unknown) => Decision::Ask {
            because: "this agent asked for something this version does not recognise, so it is \
                      being shown to you rather than decided for you."
                .into(),
        },
    }
}

/// Whether a path is inside the project.
///
/// `§9.3`: an agent asking to read `~/.ssh/id_rsa` gets a refusal and a
/// visible warning, not a prompt. Everything is resolved before comparing,
/// because `project/../../../etc/passwd` is inside the project only if nobody
/// looks.
pub fn within_project(root: &Path, requested: &Path) -> bool {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    // A path that does not exist yet cannot be canonicalized, and a write is
    // exactly that case. Resolve the deepest existing ancestor instead, then
    // check the remainder has no upward components.
    let resolved = match requested.canonicalize() {
        Ok(resolved) => resolved,
        Err(_) => {
            let mut existing = requested.to_path_buf();
            let mut remainder = Vec::new();
            while !existing.exists() {
                match existing.file_name() {
                    Some(name) => remainder.push(name.to_os_string()),
                    None => return false,
                }
                if !existing.pop() {
                    return false;
                }
            }
            let mut resolved = existing.canonicalize().unwrap_or(existing);
            for part in remainder.iter().rev() {
                // `..` in the not-yet-existing tail would escape after
                // creation, so it is refused rather than normalised away.
                if part == ".." {
                    return false;
                }
                resolved.push(part);
            }
            resolved
        }
    };

    resolved.starts_with(&root)
}

/// A filesystem request from an agent, judged.
pub fn judge_path(root: &Path, requested: &Path, writing: bool) -> Decision {
    if !within_project(root, requested) {
        return Decision::Refuse {
            because: format!(
                "`{}` is outside the project. An agent reaching outside the directory it was \
                 given is refused rather than asked about — that is not a decision to hand to \
                 someone in a hurry.",
                requested.display()
            ),
        };
    }
    if writing {
        // Inside the project, but a write: the tier decides, not the path.
        return Decision::Ask {
            because: format!(
                "`{}` is inside the project, and this would write to it.",
                requested.display()
            ),
        };
    }
    Decision::Allow
}

/// How to launch an agent.
///
/// `§2`: "Do not hardcode agent launch commands." Some agents use a
/// subcommand, some a flag, some a separate adapter binary — so this is data,
/// and a user-supplied command is a first-class case rather than a fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCommand {
    pub id: String,
    pub display: String,
    pub program: String,
    pub arguments: Vec<String>,
}

impl AgentCommand {
    /// The agents named for the initial release, plus the ones the ecosystem
    /// has settled on. A row here is a guess at a launch pattern; a
    /// user-supplied command overrides it.
    pub fn known() -> Vec<Self> {
        vec![
            Self {
                id: "claude-code".into(),
                display: "Claude Code".into(),
                program: "claude-code-acp".into(),
                arguments: Vec::new(),
            },
            Self {
                id: "codex".into(),
                display: "Codex CLI".into(),
                program: "codex-acp".into(),
                arguments: Vec::new(),
            },
            Self {
                id: "gemini".into(),
                display: "Gemini CLI".into(),
                program: "gemini".into(),
                arguments: vec!["--experimental-acp".into()],
            },
            Self {
                id: "goose".into(),
                display: "Goose".into(),
                program: "goose".into(),
                arguments: vec!["acp".into()],
            },
        ]
    }

    /// Whether the program is on the path.
    ///
    /// Shown in the picker as "installed" or "install", so a user is not
    /// offered a reasoner that cannot start.
    pub fn is_installed(&self) -> bool {
        which(&self.program).is_some()
    }
}

/// Find a program on `PATH`.
fn which(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|directory| directory.join(program))
            .find(|candidate| candidate.is_file())
    })
}

/// Turn one `session/update` notification into a transcript entry.
///
/// `§9.2`: ACP's updates map directly onto transcript rendering, and Mode A
/// emits the same shape — so one view renders both. Everything from here is
/// `Origin::External`, which is how `§1`'s weaker grounding guarantee survives
/// into what the user sees.
pub fn transcript_event(update: &Value) -> Option<TranscriptEvent> {
    let kind = update.get("sessionUpdate").and_then(Value::as_str)?;

    match kind {
        "agent_message_chunk" => Some(TranscriptEvent::Message {
            text: text_of(update.get("content")?)?,
            origin: Origin::External,
        }),
        "agent_thought_chunk" => Some(TranscriptEvent::Thought {
            text: text_of(update.get("content")?)?,
            origin: Origin::External,
        }),
        "tool_call" => Some(TranscriptEvent::ToolCall {
            id: update.get("toolCallId").and_then(Value::as_str)?.to_string(),
            tool: update.get("title").and_then(Value::as_str).unwrap_or("a tool").to_string(),
            arguments: update.get("rawInput").cloned().unwrap_or_else(|| json!({})),
            origin: Origin::External,
        }),
        "tool_call_update" => {
            use binmap_core::transcript::CallStatus;
            let status = match update.get("status").and_then(Value::as_str) {
                Some("completed") => CallStatus::Succeeded,
                Some("failed") => CallStatus::Failed,
                Some("in_progress") => CallStatus::Running,
                _ => CallStatus::Pending,
            };
            Some(TranscriptEvent::ToolResult {
                id: update.get("toolCallId").and_then(Value::as_str)?.to_string(),
                status,
                summary: update
                    .get("content")
                    .and_then(text_of)
                    .unwrap_or_else(|| status.label().to_string()),
                evidence: None,
            })
        }
        // A plan is the agent describing what it intends. Rendered as a
        // message rather than dropped: it is the closest thing Mode B has to
        // Mode A's hypothesis, and `§1` says the UI must be able to show that
        // the guarantee is weaker — which it cannot do by showing nothing.
        "plan" => {
            Some(TranscriptEvent::Message { text: plan_text(update)?, origin: Origin::External })
        }
        _ => None,
    }
}

/// Pull the text out of ACP's content shape, which is a block or a list.
fn text_of(content: &Value) -> Option<String> {
    if let Some(text) = content.get("text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(blocks) = content.as_array() {
        let joined: String = blocks
            .iter()
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("");
        return (!joined.is_empty()).then_some(joined);
    }
    content.as_str().map(str::to_string)
}

fn plan_text(update: &Value) -> Option<String> {
    let entries = update.get("entries")?.as_array()?;
    let lines: Vec<String> = entries
        .iter()
        .filter_map(|entry| {
            let content = entry.get("content").and_then(Value::as_str)?;
            let status = entry.get("status").and_then(Value::as_str).unwrap_or("pending");
            Some(format!("[{status}] {content}"))
        })
        .collect();
    (!lines.is_empty()).then(|| format!("Plan:\n{}", lines.join("\n")))
}
