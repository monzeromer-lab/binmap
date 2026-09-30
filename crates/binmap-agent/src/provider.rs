//! Providers as a data table (`A1.1`, `A1.4`, `DESIGN-AI §6`).
//!
//! The simplification the design leans on: OpenAI, DeepSeek, Moonshot and Z.ai
//! all expose OpenAI-compatible chat-completions endpoints, so there are two
//! implementations rather than five. Adding a provider is then a row here plus
//! a quirks record, which is what "we can add more later" is supposed to cost.
//!
//! `§6.2` is the part worth taking seriously: *OpenAI-compatible does not mean
//! OpenAI-identical.* The quirks are not trivia, they are the differences that
//! break an agent loop in production — malformed tool arguments under pressure,
//! no parallel calls, reasoning models that refuse a system prompt, context
//! windows an order of magnitude apart. Encoding them here is how the loop
//! avoids discovering them from a user.

use binmap_core::reasoner::{Mode, Reasoner, ReasonerChoice, Unavailable};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// Which wire format a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiShape {
    /// Claude's messages API.
    Anthropic,
    /// OpenAI's chat-completions, and everything that copied it.
    OpenAiCompatible,
    /// No model at all. The entire test suite runs on this.
    Null,
}

/// How well a provider handles being asked to call a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCallingSupport {
    /// Declared functions, returned as structured calls.
    Native,
    /// No function calling; tools go in the prompt and come back as JSON that
    /// has to be parsed out of prose.
    JsonMode,
    None,
}

/// What a model can do, read rather than assumed.
///
/// `§6.2`: the context strategy must read `context_window` rather than
/// hardcoding a number, because these differ by an order of magnitude and a
/// prompt built for one silently overflows another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    pub context_window: usize,
    pub max_output: usize,
    pub tool_calling: ToolCallingSupport,
    pub parallel_tool_calls: bool,
    pub reasoning_effort: bool,
    pub vision: bool,
    /// (input, output) per million tokens, for the cost meter. `None` for a
    /// local model, which is free — and the meter must say "free" rather than
    /// "0.00", because they mean different things to a reader.
    pub cost_per_mtok: Option<(f64, f64)>,
}

impl Capabilities {
    /// What one step costs, given what it used.
    ///
    /// `None` when the model has no price, which is not the same as zero.
    pub fn cost_of(&self, input_tokens: u64, output_tokens: u64) -> Option<f64> {
        let (input, output) = self.cost_per_mtok?;
        Some((input_tokens as f64 * input + output_tokens as f64 * output) / 1_000_000.0)
    }

    /// Whether this model can be driven by the Mode A loop at all.
    ///
    /// A model that cannot call a tool cannot participate in a loop whose
    /// every step is a tool call. Better to say so in the picker than to fail
    /// on step one.
    pub fn can_drive_the_loop(&self) -> bool {
        !matches!(self.tool_calling, ToolCallingSupport::None)
    }
}

/// The differences that break loops (`§6.2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Quirks {
    /// Emits schema-valid-looking arguments that are not. The loop validates
    /// every call and retries once with the error fed back; this flag is why
    /// that path exists rather than being paranoia.
    pub malformed_tool_arguments: bool,
    /// Rejects or ignores a system prompt — common among reasoning models.
    /// The prompt has to be folded into the first user message instead.
    pub no_system_prompt: bool,
    /// Rejects an explicit temperature.
    pub fixed_temperature: bool,
    /// Will not interleave tool calls with reasoning content, so a step that
    /// expects a hypothesis *and* a call in one response will not get both.
    pub no_tools_with_reasoning: bool,
}

impl Quirks {
    pub const NONE: Self = Self {
        malformed_tool_arguments: false,
        no_system_prompt: false,
        fixed_temperature: false,
        no_tools_with_reasoning: false,
    };

    /// Whether the hypothesis and the tool call have to be separate turns.
    ///
    /// `§8` wants a hypothesis before every tool call. Where a model will not
    /// produce both at once, the loop must ask twice rather than conclude the
    /// model refused to hypothesise.
    pub fn needs_separate_hypothesis_turn(&self) -> bool {
        self.no_tools_with_reasoning
    }
}

/// One model a provider offers.
///
/// Serialize but not Deserialize, and for both a `ModelSpec` and a
/// `ProviderSpec` the reason is the same: this is a compile-time table of
/// `&'static str`, so it can be shown, logged and exported, but there is
/// nothing to read one back *into*. A provider that could be deserialised
/// would be a provider a config file could invent.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelSpec {
    /// `Cow` rather than `&'static str` because a local runner hosts whatever
    /// its owner pulled: the table's own rows are static, and a name typed by
    /// the user is owned. The alternative was leaking the string.
    pub id: Cow<'static, str>,
    pub display: Cow<'static, str>,
    pub capabilities: Capabilities,
}

/// One provider. See `ModelSpec` on why this serialises but does not
/// deserialise.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderSpec {
    pub id: &'static str,
    pub display: &'static str,
    pub shape: ApiShape,
    pub default_base_url: &'static str,
    /// Read as a fallback when the keyring has nothing. `§6.3` allows it
    /// because that is how CI and a lot of developers already work.
    pub key_env: &'static str,
    pub models: &'static [ModelSpec],
    pub quirks: Quirks,
    /// Whether using this sends your code to someone else's machine.
    ///
    /// `§6.3` lets a project set `allow_cloud_models = false`, and `§11` is
    /// about what leaves the machine. Neither is expressible unless each row
    /// says which it is.
    pub cloud: bool,
}

impl ProviderSpec {
    /// The model to use when the user has not chosen one.
    pub fn default_model(&self) -> Option<&ModelSpec> {
        self.models.first()
    }

    pub fn model(&self, id: &str) -> Option<&ModelSpec> {
        self.models.iter().find(|model| model.id == id)
    }

    /// Whether this provider will accept a model name that is not in the table.
    ///
    /// A local runner hosts whatever its owner pulled. Listing one name and
    /// refusing every other made the table a whitelist of models we happen to
    /// have heard of, which for a local runner is nobody's business but the
    /// owner's — someone with `llama3.2` pulled could not select it.
    ///
    /// Cloud providers stay closed: there the name is billed against an
    /// account, and a typo should be caught here rather than charged for.
    pub fn accepts_any_model(&self) -> bool {
        !self.cloud
    }

    /// The spec to use for `id`, inventing one for an unlisted local model.
    ///
    /// The invented capabilities are deliberately conservative — a small
    /// context and no parallel tool calls — because we know nothing about a
    /// model we have never seen, and over-promising its context window is how
    /// a prompt silently overflows.
    pub fn model_or_unlisted(&self, id: &str) -> Option<ModelSpec> {
        if let Some(known) = self.model(id) {
            return Some(known.clone());
        }
        if !self.accepts_any_model() {
            return None;
        }
        let fallback = self.default_model()?;
        Some(ModelSpec {
            id: Cow::Owned(id.to_string()),
            display: Cow::Owned(format!("{id} (unlisted)")),
            capabilities: Capabilities {
                // Whatever the runner actually has, assume the smaller of what
                // we list and something modest.
                context_window: fallback.capabilities.context_window.min(8_192),
                max_output: fallback.capabilities.max_output.min(4_096),
                parallel_tool_calls: false,
                ..fallback.capabilities.clone()
            },
        })
    }
}

const fn window(context_window: usize, max_output: usize) -> Capabilities {
    Capabilities {
        context_window,
        max_output,
        tool_calling: ToolCallingSupport::Native,
        parallel_tool_calls: true,
        reasoning_effort: false,
        vision: false,
        cost_per_mtok: None,
    }
}

/// Claude.
const ANTHROPIC_MODELS: &[ModelSpec] = &[ModelSpec {
    id: Cow::Borrowed("claude-sonnet-5-5"),
    display: Cow::Borrowed("Claude Sonnet 5.5"),
    capabilities: Capabilities {
        reasoning_effort: true,
        vision: true,
        cost_per_mtok: Some((3.0, 15.0)),
        ..window(200_000, 64_000)
    },
}];

const OPENAI_MODELS: &[ModelSpec] = &[ModelSpec {
    id: Cow::Borrowed("gpt-5"),
    display: Cow::Borrowed("GPT-5"),
    capabilities: Capabilities {
        reasoning_effort: true,
        vision: true,
        cost_per_mtok: Some((1.25, 10.0)),
        ..window(400_000, 128_000)
    },
}];

const DEEPSEEK_MODELS: &[ModelSpec] = &[ModelSpec {
    id: Cow::Borrowed("deepseek-reasoner"),
    display: Cow::Borrowed("DeepSeek R1"),
    capabilities: Capabilities {
        reasoning_effort: true,
        // The reasoner will not interleave tool calls with its reasoning, so
        // the loop asks for the hypothesis in its own turn. See `Quirks`.
        parallel_tool_calls: false,
        cost_per_mtok: Some((0.55, 2.19)),
        ..window(128_000, 32_000)
    },
}];

const MOONSHOT_MODELS: &[ModelSpec] = &[ModelSpec {
    id: Cow::Borrowed("kimi-k2"),
    display: Cow::Borrowed("Kimi K2"),
    capabilities: Capabilities { cost_per_mtok: Some((0.6, 2.5)), ..window(128_000, 16_000) },
}];

const ZAI_MODELS: &[ModelSpec] = &[ModelSpec {
    id: Cow::Borrowed("glm-4.6"),
    display: Cow::Borrowed("GLM-4.6"),
    capabilities: Capabilities { cost_per_mtok: Some((0.6, 2.2)), ..window(200_000, 32_000) },
}];

/// A local runner, which is the one Phase 1 is authored against.
///
/// `§7.3`: prompts are written for the weakest and most privacy-preserving
/// model, not for a frontier one. A prompt that only works on Claude is a
/// prompt that will embarrass us on a laptop.
const LOCAL_MODELS: &[ModelSpec] = &[ModelSpec {
    id: Cow::Borrowed("qwen3-coder"),
    display: Cow::Borrowed("Qwen3 Coder (local)"),
    capabilities: Capabilities {
        parallel_tool_calls: false,
        // Free, which `cost_per_mtok: None` means — as distinct from costing
        // nothing per token, which is what `Some((0.0, 0.0))` would claim.
        cost_per_mtok: None,
        ..window(32_768, 8_192)
    },
}];

/// Every provider, as a table.
pub const PROVIDERS: &[ProviderSpec] = &[
    ProviderSpec {
        id: "local",
        display: "Local (OpenAI-compatible)",
        shape: ApiShape::OpenAiCompatible,
        default_base_url: "http://localhost:11434/v1",
        key_env: "BINMAP_LOCAL_API_KEY",
        models: LOCAL_MODELS,
        // A local runner is the least reliable at tool calling, which is
        // exactly why it is the one the loop is developed against.
        quirks: Quirks { malformed_tool_arguments: true, ..Quirks::NONE },
        cloud: false,
    },
    ProviderSpec {
        id: "anthropic",
        display: "Claude",
        shape: ApiShape::Anthropic,
        default_base_url: "https://api.anthropic.com/v1",
        key_env: "ANTHROPIC_API_KEY",
        models: ANTHROPIC_MODELS,
        quirks: Quirks::NONE,
        cloud: true,
    },
    ProviderSpec {
        id: "openai",
        display: "OpenAI",
        shape: ApiShape::OpenAiCompatible,
        default_base_url: "https://api.openai.com/v1",
        key_env: "OPENAI_API_KEY",
        models: OPENAI_MODELS,
        quirks: Quirks { fixed_temperature: true, ..Quirks::NONE },
        cloud: true,
    },
    ProviderSpec {
        id: "deepseek",
        display: "DeepSeek",
        shape: ApiShape::OpenAiCompatible,
        default_base_url: "https://api.deepseek.com/v1",
        key_env: "DEEPSEEK_API_KEY",
        models: DEEPSEEK_MODELS,
        quirks: Quirks { no_tools_with_reasoning: true, fixed_temperature: true, ..Quirks::NONE },
        cloud: true,
    },
    ProviderSpec {
        id: "moonshot",
        display: "Kimi",
        shape: ApiShape::OpenAiCompatible,
        default_base_url: "https://api.moonshot.cn/v1",
        key_env: "MOONSHOT_API_KEY",
        models: MOONSHOT_MODELS,
        quirks: Quirks { malformed_tool_arguments: true, ..Quirks::NONE },
        cloud: true,
    },
    ProviderSpec {
        id: "zai",
        display: "Z.ai",
        shape: ApiShape::OpenAiCompatible,
        default_base_url: "https://api.z.ai/api/paas/v4",
        key_env: "ZAI_API_KEY",
        models: ZAI_MODELS,
        quirks: Quirks { malformed_tool_arguments: true, ..Quirks::NONE },
        cloud: true,
    },
];

/// Look a provider up.
pub fn provider(id: &str) -> Option<&'static ProviderSpec> {
    PROVIDERS.iter().find(|spec| spec.id == id)
}

/// The providers a project is allowed to use.
///
/// `§6.3`: when a project sets `allow_cloud_models = false` the cloud entries
/// are not merely hidden but unselectable with the reason shown — so this
/// returns them alongside why, rather than filtering them out and leaving the
/// picker unable to explain the absence.
pub fn selectable(allow_cloud: bool) -> Vec<(&'static ProviderSpec, Option<&'static str>)> {
    PROVIDERS
        .iter()
        .map(|spec| {
            let refusal = if spec.cloud && !allow_cloud {
                Some("this project does not allow cloud models")
            } else {
                None
            };
            (spec, refusal)
        })
        .collect()
}

/// Whether a key for this provider is present, without reading its value.
///
/// `§6.3`: the picker shows which keys are present without displaying them.
/// Returning a bool rather than the key is how that is kept true by
/// construction.
pub fn key_present(spec: &ProviderSpec) -> bool {
    // A local runner needs no key, so its absence is not a reason to grey it
    // out — that would hide the one provider that always works.
    if !spec.cloud {
        return true;
    }
    std::env::var(spec.key_env).map(|key| !key.trim().is_empty()).unwrap_or(false)
}

/// The table, as the picker reads it (`U1.3`).
///
/// This is the one place the provider table becomes interface vocabulary. The
/// interface cannot see `ProviderSpec` — `§2.4` forbids it depending on this
/// crate at all — so it receives `binmap_core::reasoner::Reasoner`, and every
/// base URL, key variable and quirk stays here where it belongs.
///
/// `allow_cloud` comes from the project, and a forbidden row is returned
/// carrying its reason rather than omitted (`§6.3`).
pub fn reasoners(allow_cloud: bool) -> Vec<Reasoner> {
    let mut offered = Vec::new();

    for spec in PROVIDERS {
        for model in spec.models {
            // The first reason that applies is the one shown. Cloud-forbidden
            // outranks a missing key: telling someone to set a key for a
            // provider the project will not permit is advice that cannot help.
            let unavailable = if spec.cloud && !allow_cloud {
                Some(Unavailable::CloudForbidden)
            } else if !key_present(spec) {
                Some(Unavailable::NoCredential { variable: spec.key_env.to_string() })
            } else {
                None
            };

            offered.push(Reasoner {
                id: format!("{}/{}", spec.id, model.id),
                display: format!("{} · {}", spec.display, model.display),
                mode: Mode::Native,
                cloud: spec.cloud,
                unavailable,
                cost_per_mtok: model.capabilities.cost_per_mtok,
            });
        }
    }

    // `§9.1`: "None" is a first-class choice listed alongside the others rather
    // than hidden in settings.
    offered.push(Reasoner::none());
    offered
}

/// The picker's initial state.
pub fn choice(allow_cloud: bool) -> ReasonerChoice {
    ReasonerChoice { available: reasoners(allow_cloud), selected: Reasoner::none().id }
}
