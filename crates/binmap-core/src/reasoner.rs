//! Who is going to think about this (`U1.3`, `DESIGN-AI §9.1`).
//!
//! The picker spans both modes, because from the user's point of view they are
//! making one choice. So this is a description of a choice, not of a provider:
//! a native model, an external agent over ACP, and "None" all arrive here as
//! the same shape, and the panel renders a list rather than two lists it has to
//! reconcile.
//!
//! In `binmap-core` for the reason given in `transcript.rs`: `§2.4` forbids the
//! interface from depending on `binmap-agent`, so anything the Agent panel
//! renders must have its shape here. The provider *table* — base URLs, quirks,
//! wire shapes — stays in `binmap-agent`, because none of that is the
//! interface's business.

use serde::{Deserialize, Serialize};

/// How a reasoner is driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Our own loop. Forces a hypothesis, enforces the gate.
    Native,
    /// An external agent over ACP. `§1`: a weaker grounding guarantee, and the
    /// interface must say so rather than presenting the two as equivalent.
    External,
    /// No model. A first-class choice listed beside the others rather than
    /// hidden in settings (`§9.1`), because every analysis works without one.
    None,
}

impl Mode {
    /// The prefix the picker shows.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Native => "Native",
            Mode::External => "ACP",
            Mode::None => "None",
        }
    }

    /// Whether claims from this need the external-origin badge.
    pub fn needs_badge(self) -> bool {
        matches!(self, Mode::External)
    }
}

/// Why a reasoner cannot be chosen.
///
/// Carried rather than filtered, because `§6.3` says a forbidden cloud provider
/// is "not merely hidden but unselectable, with the reason shown". A picker
/// that dropped the row could not explain the absence, and the user would
/// conclude the application was broken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unavailable {
    /// The project forbids sending code off the machine.
    CloudForbidden,
    /// No key is configured. `§6.3`: presence is shown, the value never is.
    NoCredential { variable: String },
    /// An ACP agent that is not installed.
    NotInstalled { install_with: String },
    /// A wire shape we do not speak yet.
    NotImplemented { arrives_in: String },
}

impl Unavailable {
    /// The sentence shown beside the greyed-out row.
    pub fn describe(&self) -> String {
        match self {
            Unavailable::CloudForbidden => {
                "this project does not allow cloud models, so nothing here leaves the machine"
                    .into()
            }
            Unavailable::NoCredential { variable } => {
                format!("no key configured — set {variable}, or use a local model")
            }
            Unavailable::NotInstalled { install_with } => {
                format!("not installed — {install_with}")
            }
            Unavailable::NotImplemented { arrives_in } => {
                format!("not supported yet — arrives in {arrives_in}")
            }
        }
    }
}

/// One entry in the picker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reasoner {
    /// Stable, and what a session records. `"local/qwen3-coder"`, `"none"`.
    pub id: String,
    /// What the row says.
    pub display: String,
    pub mode: Mode,
    /// Whether choosing this sends your code to someone else's machine.
    ///
    /// `§11` is about egress, and the picker is the last place a user can
    /// decide against it, so the fact has to be visible at the point of choice
    /// rather than in a settings page.
    pub cloud: bool,
    /// `None` when it can be chosen.
    pub unavailable: Option<Unavailable>,
    /// (input, output) per million tokens, for the cost meter. `None` means
    /// free — which is not the same as zero, and the meter says so.
    pub cost_per_mtok: Option<(f64, f64)>,
}

impl Reasoner {
    /// The deterministic choice, which is always available.
    pub fn none() -> Self {
        Self {
            id: "none".into(),
            display: "None (deterministic only)".into(),
            mode: Mode::None,
            cloud: false,
            unavailable: None,
            cost_per_mtok: None,
        }
    }

    pub fn is_selectable(&self) -> bool {
        self.unavailable.is_none()
    }

    /// What the cost meter shows before anything has been spent.
    ///
    /// "Free" and "$0.00" mean different things to a reader: one is a local
    /// model, the other is a priced model nobody has used yet.
    pub fn price_label(&self) -> String {
        match self.cost_per_mtok {
            None if self.mode == Mode::None => "no model".into(),
            None => "free".into(),
            Some((input, output)) => format!("${input:.2}/${output:.2} per Mtok"),
        }
    }

    /// The full row, as the picker reads it.
    pub fn describe(&self) -> String {
        let mut line = format!("{} · {}", self.mode.label(), self.display);
        if let Some(reason) = &self.unavailable {
            line.push_str(" — ");
            line.push_str(&reason.describe());
        }
        line
    }
}

/// Everything offered, and what is chosen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasonerChoice {
    pub available: Vec<Reasoner>,
    /// The id of the chosen one. Defaults to `"none"`, because `§7`'s phase
    /// table has the product working with no model at all and a default that
    /// reached for one would contradict it.
    pub selected: String,
}

impl Default for ReasonerChoice {
    fn default() -> Self {
        Self { available: vec![Reasoner::none()], selected: "none".into() }
    }
}

impl ReasonerChoice {
    pub fn selected(&self) -> Option<&Reasoner> {
        self.available.iter().find(|reasoner| reasoner.id == self.selected)
    }

    /// Choose one, or say why not.
    ///
    /// Refuses an unavailable row rather than selecting it and failing later:
    /// the reason is already known here, and discovering it at the first model
    /// call would report it as a session failure instead of a setup problem.
    pub fn select(&mut self, id: &str) -> Result<(), String> {
        let reasoner = self
            .available
            .iter()
            .find(|reasoner| reasoner.id == id)
            .ok_or_else(|| format!("there is no reasoner `{id}`"))?;
        if let Some(reason) = &reasoner.unavailable {
            return Err(format!("{} cannot be used: {}", reasoner.display, reason.describe()));
        }
        self.selected = id.to_string();
        Ok(())
    }

    /// Whether a model would be called at all.
    pub fn uses_a_model(&self) -> bool {
        self.selected().map(|reasoner| reasoner.mode != Mode::None).unwrap_or(false)
    }
}
