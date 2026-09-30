//! What a model is, to us (`A1.1`, `DESIGN-AI §6.1`).
//!
//! The trait is **blocking**, where the design sketches it as `async_trait`,
//! and that is a deliberate departure worth stating. Nothing else in this
//! workspace is async: the engine builds, measures and gates synchronously,
//! and the Mode A loop is sequential by its own definition — hypothesis, then
//! tool, then revision, each needing the last. Introducing an async runtime to
//! the engine would buy parallel tool calls, which `§6.2` says are not
//! universally supported and which the loop must serialize without anyway. A
//! blocking call on a worker thread is what the rest of the product already
//! does with cargo, hyperfine and everything else that takes time.

use crate::provider::{Capabilities, ProviderSpec};
use crate::registry::{Tool, ToolCall};
use binmap_core::error::{Error, Result};
use serde::{Deserialize, Serialize};

/// Who said something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
    /// The result of a tool call, fed back.
    Tool,
}

/// One turn in a conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// Set on a `Tool` message, naming the call it answers. Providers match
    /// results to calls by this, and omitting it makes the model see an
    /// unattached result.
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: Role::System, content: content.into(), tool_call_id: None }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into(), tool_call_id: None }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into(), tool_call_id: None }
    }

    /// A tool's output, answering a specific call.
    pub fn tool(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self { role: Role::Tool, content: content.into(), tool_call_id: Some(id.into()) }
    }
}

/// What we ask a model for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    /// The tools it may call. Rendered into whatever the provider's shape
    /// wants, from the one registry.
    pub tools: Vec<Tool>,
    pub max_output: usize,
    /// `None` where the provider refuses an explicit temperature
    /// (`Quirks::fixed_temperature`).
    pub temperature: Option<f32>,
}

impl CompletionRequest {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
            tools: Vec::new(),
            max_output: 4096,
            temperature: None,
        }
    }

    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = tools;
        self
    }

    /// Fold the system prompt into the first user message.
    ///
    /// For `Quirks::no_system_prompt`. A reasoning model that ignores a system
    /// role would otherwise silently lose the entire briefing — the failure
    /// looks like a model that will not follow instructions.
    pub fn without_system_role(mut self) -> Self {
        let system: Vec<String> = self
            .messages
            .iter()
            .filter(|message| message.role == Role::System)
            .map(|message| message.content.clone())
            .collect();
        if system.is_empty() {
            return self;
        }
        self.messages.retain(|message| message.role != Role::System);
        let preamble = system.join("\n\n");

        match self.messages.iter_mut().find(|message| message.role == Role::User) {
            Some(first) => first.content = format!("{preamble}\n\n{}", first.content),
            // No user turn to fold into: it becomes one, because dropping the
            // briefing entirely is the worse failure.
            None => self.messages.insert(0, Message::user(preamble)),
        }
        self
    }
}

/// How many tokens a step cost.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl Usage {
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

impl std::ops::AddAssign for Usage {
    fn add_assign(&mut self, other: Self) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
    }
}

/// Why the model stopped talking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinishReason {
    /// It finished its turn.
    Stop,
    /// It wants to call tools.
    ToolCalls,
    /// It hit `max_output` mid-sentence, so the content is truncated and must
    /// not be parsed as though it were complete.
    Length,
}

/// What came back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Prose. May be empty when the model only wanted to call something.
    pub content: String,
    /// Reasoning content, where the provider exposes it separately.
    pub reasoning: Option<String>,
    /// What it wants to call, with the provider's own ids preserved so results
    /// can be matched back.
    pub calls: Vec<IdentifiedCall>,
    pub usage: Usage,
    pub finish_reason: FinishReason,
}

/// A tool call with the provider's id attached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdentifiedCall {
    pub id: String,
    pub call: ToolCall,
}

/// A model, whatever it actually is.
pub trait ModelBackend: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> &Capabilities;
    /// Blocking. See the note at the top of this module.
    fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;
}

/// No model at all.
///
/// `§6.1` lists this alongside the real backends rather than as a test double,
/// and `§7`'s phase table has Phase 0 running on it exclusively. The whole
/// suite uses it, which is what keeps the deterministic core deterministic:
/// if a test could reach a model, the test would not be a test.
#[derive(Debug, Clone)]
pub struct NullBackend {
    capabilities: Capabilities,
}

impl Default for NullBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl NullBackend {
    pub fn new() -> Self {
        Self {
            capabilities: Capabilities {
                context_window: 0,
                max_output: 0,
                tool_calling: crate::provider::ToolCallingSupport::None,
                parallel_tool_calls: false,
                reasoning_effort: false,
                vision: false,
                cost_per_mtok: None,
            },
        }
    }
}

impl ModelBackend for NullBackend {
    fn id(&self) -> &str {
        "null"
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Refuses, in as many words.
    ///
    /// Not an empty response: a backend that returns nothing successfully
    /// would let a loop run to its step budget producing nothing and call that
    /// a result. Refusing at the first call is how "no model configured"
    /// reaches the user as a sentence rather than as an empty panel.
    fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse> {
        Err(Error::Other(
            "no reasoner is selected, so there is nothing to ask. Pick one in the Agent panel, \
             or keep working deterministically — every analysis in this phase runs without a \
             model."
                .into(),
        ))
    }
}

/// Build the backend a provider row describes.
///
/// The point of the table in `provider.rs` is that this function is the only
/// place that has to know there are two shapes rather than five providers.
pub fn backend_for(
    spec: &ProviderSpec,
    model: &str,
    transport: std::sync::Arc<dyn crate::openai::HttpTransport>,
) -> Result<Box<dyn ModelBackend>> {
    // A local runner hosts whatever its owner pulled, so an unlisted name is
    // accepted there and refused for a provider that bills for it.
    let model_spec = spec.model_or_unlisted(model).ok_or_else(|| {
        let known: Vec<&str> = spec.models.iter().map(|model| model.id.as_ref()).collect();
        Error::Other(format!(
            "{} does not offer a model `{model}`. It has: {}",
            spec.display,
            known.join(", ")
        ))
    })?;

    match spec.shape {
        crate::provider::ApiShape::OpenAiCompatible => {
            Ok(Box::new(crate::openai::OpenAiCompatibleBackend::new(spec, &model_spec, transport)))
        }
        // Claude's own shape is Phase 1.5's row to add; until it exists,
        // saying so is better than quietly sending Anthropic an
        // OpenAI-shaped body and reporting whatever error it returns.
        crate::provider::ApiShape::Anthropic => Err(Error::Other(format!(
            "{} speaks Anthropic's messages API, which arrives in Phase 1.5. Every \
             OpenAI-compatible provider works now, including a local runner.",
            spec.display
        ))),
        crate::provider::ApiShape::Null => Ok(Box::new(NullBackend::new())),
    }
}
