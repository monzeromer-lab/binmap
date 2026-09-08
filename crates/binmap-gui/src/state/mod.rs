//! What the interface knows, as plain data.
//!
//! Deliberately free of any framework type. The plan's testing strategy asks
//! for "view state against a fake engine returning scripted events", and that
//! is only cheap if the state can be driven without a window — so it is.

use binmap_core::capability::{Capabilities, Capability};
use binmap_core::event::{EngineEvent, RunId};
use binmap_core::facade::{Probe, ProbeStatus};
use binmap_core::finding::{Finding, FindingKind};
use binmap_core::traits::{Target, TargetFamily};
use std::collections::BTreeMap;

/// The views the nav rail can offer.
///
/// A view a target cannot support is **absent** from the rail rather than
/// present and empty (§2.5). That is a different failure from a blocked
/// action, which states its reason instead of disappearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum View {
    Target,
    Size,
    Tune,
    Failure,
    Perf,
    Load,
    Replay,
    Agent,
    Environment,
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Target => "Target",
            View::Size => "Size",
            View::Tune => "Tune",
            View::Failure => "Failure",
            View::Perf => "Perf",
            View::Load => "Load",
            View::Replay => "Replay",
            View::Agent => "Agent",
            View::Environment => "Environment",
        }
    }

    /// The capability this view needs. `None` for the views that are always
    /// available whatever the target is.
    pub fn requires(self) -> Option<Capability> {
        match self {
            View::Target | View::Agent | View::Environment => None,
            View::Size => Some(Capability::SizeAttribution),
            View::Tune => Some(Capability::ConfigurationSweep),
            View::Failure => Some(Capability::CrashAnalysis),
            View::Perf => Some(Capability::PerformanceAttribution),
            View::Load => Some(Capability::LoadTime),
            View::Replay => Some(Capability::ReplayDebugging),
        }
    }

    /// In nav rail order.
    pub const ALL: [View; 9] = [
        View::Target,
        View::Size,
        View::Tune,
        View::Failure,
        View::Perf,
        View::Load,
        View::Replay,
        View::Agent,
        View::Environment,
    ];
}

/// One entry in the nav rail, with the count the design puts beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavEntry {
    pub view: View,
    pub findings: usize,
}

/// A run in flight, as the progress indicator reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunProgress {
    pub run: RunId,
    pub description: String,
    pub completed: usize,
    pub total: Option<usize>,
    /// The last thing that happened, shown under the bar.
    pub message: String,
    pub state: RunPhase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunPhase {
    Running,
    Finished {
        summary: String,
    },
    /// Cancelled, keeping everything measured. Not an error.
    Cancelled {
        completed: usize,
    },
    Failed {
        error: String,
    },
}

impl RunProgress {
    pub fn is_running(&self) -> bool {
        matches!(self.state, RunPhase::Running)
    }

    /// The fraction complete, where the total is known. `None` renders as an
    /// indeterminate bar rather than as a guess.
    pub fn fraction(&self) -> Option<f32> {
        let total = self.total?;
        if total == 0 {
            return None;
        }
        Some((self.completed as f32 / total as f32).clamp(0.0, 1.0))
    }
}

/// Everything one window shows.
#[derive(Debug, Clone, Default)]
pub struct AppState {
    targets: Vec<Target>,
    selected_target: Option<String>,
    view: Option<View>,
    findings: Vec<Finding>,
    /// Findings by id, so selection survives a re-sort.
    selected_finding: Option<String>,
    runs: BTreeMap<RunId, RunProgress>,
    probes: Vec<Probe>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    // -- targets ----------------------------------------------------------

    pub fn set_targets(&mut self, targets: Vec<Target>) {
        self.targets = targets;
        if self.selected_target.is_none() {
            self.selected_target = self.targets.first().map(|target| target.id.clone());
        }
        self.view = self.view.filter(|view| self.nav_entries().iter().any(|e| e.view == *view));
    }

    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    /// Targets grouped by language, in the order the project view lists them.
    pub fn targets_by_family(&self) -> Vec<(TargetFamily, Vec<&Target>)> {
        let mut groups: BTreeMap<TargetFamily, Vec<&Target>> = BTreeMap::new();
        for target in &self.targets {
            groups.entry(target.family).or_default().push(target);
        }
        groups.into_iter().collect()
    }

    pub fn select_target(&mut self, id: &str) -> bool {
        if !self.targets.iter().any(|target| target.id == id) {
            return false;
        }
        self.selected_target = Some(id.to_string());
        // A view the new target cannot support must not stay selected.
        if self.view.is_some_and(|view| !self.nav_entries().iter().any(|e| e.view == view)) {
            self.view = None;
        }
        true
    }

    pub fn selected_target(&self) -> Option<&Target> {
        let id = self.selected_target.as_ref()?;
        self.targets.iter().find(|target| &target.id == id)
    }

    fn capabilities(&self) -> Capabilities {
        self.selected_target().map(|target| target.capabilities.clone()).unwrap_or_default()
    }

    // -- the nav rail -----------------------------------------------------

    /// The entries the rail shows for the selected target.
    ///
    /// Entries whose capability the target lacks are simply not here. That is
    /// the rule, and it is why this returns a list rather than a list of
    /// enabled-or-not.
    pub fn nav_entries(&self) -> Vec<NavEntry> {
        let capabilities = self.capabilities();
        View::ALL
            .into_iter()
            .filter(|view| match view.requires() {
                None => true,
                Some(capability) => capabilities.has(capability),
            })
            .map(|view| NavEntry { view, findings: self.findings_for(view).len() })
            .collect()
    }

    pub fn view(&self) -> Option<View> {
        self.view
    }

    /// Select a view. Refuses a view the target cannot support rather than
    /// showing an empty one.
    pub fn select_view(&mut self, view: View) -> bool {
        if !self.nav_entries().iter().any(|entry| entry.view == view) {
            return false;
        }
        self.view = Some(view);
        true
    }

    // -- findings ---------------------------------------------------------

    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// The findings that belong under one view.
    pub fn findings_for(&self, view: View) -> Vec<&Finding> {
        self.findings.iter().filter(|finding| view_for_kind(finding.kind()) == Some(view)).collect()
    }

    pub fn select_finding(&mut self, id: &str) -> bool {
        if !self.findings.iter().any(|finding| finding.id() == id) {
            return false;
        }
        self.selected_finding = Some(id.to_string());
        true
    }

    /// What the Inspector is showing. Always present as a panel; empty is a
    /// state it renders rather than a reason to hide.
    pub fn selected_finding(&self) -> Option<&Finding> {
        let id = self.selected_finding.as_ref()?;
        self.findings.iter().find(|finding| finding.id() == id)
    }

    // -- runs -------------------------------------------------------------

    pub fn runs(&self) -> impl Iterator<Item = &RunProgress> {
        self.runs.values()
    }

    pub fn run(&self, run: &RunId) -> Option<&RunProgress> {
        self.runs.get(run)
    }

    pub fn is_busy(&self) -> bool {
        self.runs.values().any(RunProgress::is_running)
    }

    // -- probes -----------------------------------------------------------

    pub fn set_probes(&mut self, probes: Vec<Probe>) {
        self.probes = probes;
    }

    pub fn probes(&self) -> &[Probe] {
        &self.probes
    }

    /// Probes grouped as the environment panel renders them, in the order they
    /// were reported.
    pub fn probes_by_group(&self) -> Vec<(&str, Vec<&Probe>)> {
        let mut groups: Vec<(&str, Vec<&Probe>)> = Vec::new();
        for probe in &self.probes {
            match groups.iter_mut().find(|(name, _)| *name == probe.group) {
                Some((_, entries)) => entries.push(probe),
                None => groups.push((probe.group.as_str(), vec![probe])),
            }
        }
        groups
    }

    /// How many probes need attention. The badge on the Environment entry.
    pub fn unmet_probes(&self) -> usize {
        self.probes.iter().filter(|probe| probe.status != ProbeStatus::Present).count()
    }

    // -- the event model --------------------------------------------------

    /// Apply one engine event.
    ///
    /// This is the whole of how the interface learns anything. Findings appear
    /// as they are discovered, and a cancelled run keeps everything it
    /// measured — both of which are properties of this function rather than
    /// promises made elsewhere.
    pub fn apply(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::Started { run, description, total } => {
                self.runs.insert(
                    run.clone(),
                    RunProgress {
                        run,
                        description,
                        completed: 0,
                        total,
                        message: String::new(),
                        state: RunPhase::Running,
                    },
                );
            }
            EngineEvent::Progress { run, completed, message } => {
                if let Some(progress) = self.runs.get_mut(&run) {
                    // Progress never goes backwards: events from concurrent
                    // workers can arrive out of order, and a bar that jitters
                    // reads as a bug in the measurement.
                    progress.completed = progress.completed.max(completed);
                    progress.message = message;
                }
            }
            EngineEvent::Finding { finding, .. } => {
                let finding = *finding;
                match self.findings.iter_mut().find(|existing| existing.id() == finding.id()) {
                    Some(existing) => *existing = finding,
                    None => {
                        // The first finding of a run selects itself, so the
                        // Inspector has something to show without a click.
                        if self.selected_finding.is_none() {
                            self.selected_finding = Some(finding.id().to_string());
                        }
                        self.findings.push(finding);
                    }
                }
            }
            EngineEvent::Finished { run, summary } => {
                if let Some(progress) = self.runs.get_mut(&run) {
                    if let Some(total) = progress.total {
                        progress.completed = total;
                    }
                    progress.state = RunPhase::Finished { summary };
                }
            }
            EngineEvent::Cancelled { run, completed } => {
                if let Some(progress) = self.runs.get_mut(&run) {
                    progress.completed = progress.completed.max(completed);
                    progress.state = RunPhase::Cancelled { completed };
                }
            }
            EngineEvent::Failed { run, error } => {
                if let Some(progress) = self.runs.get_mut(&run) {
                    progress.state = RunPhase::Failed { error };
                }
            }
        }
    }
}

/// Which view a finding belongs under.
fn view_for_kind(kind: &FindingKind) -> Option<View> {
    match kind {
        FindingKind::Configuration
        | FindingKind::FrontierPoint
        | FindingKind::RejectedCandidate => Some(View::Tune),
        FindingKind::SizeAttribution
        | FindingKind::Monomorphization
        | FindingKind::SizeDriver
        | FindingKind::Diff => Some(View::Size),
        FindingKind::EnvironmentProbe => Some(View::Environment),
        // FindingKind is non-exhaustive on purpose: a later phase adds kinds
        // without a schema break, and a kind this build does not recognise
        // belongs to no view rather than to the wrong one.
        FindingKind::Other(_) => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------
// Intent
// ---------------------------------------------------------------------------

/// Everything the interface can ask the application to do.
///
/// Views emit these; one place handles them. That indirection is not
/// ceremony — it is what makes `U12` ("keyboard reaches every action")
/// achievable rather than aspirational, because a key binding and a click
/// produce the same value, and the command palette is a list of them. It also
/// makes interaction testable without a window: an `Action` is plain data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Show a view. Refused if the target cannot support it.
    SelectView(View),
    /// Select a target, and drop a view it cannot support.
    SelectTarget(String),
    /// Open one configuration in the Profile Lab's panel.
    SelectConfiguration(String),
    /// Show a finding in the Inspector.
    SelectFinding(String),
    /// Switch the Inspector's tab.
    SelectTab(InspectorTab),
    /// Sweep the selected target's configuration matrix.
    StartSweep,
    /// Stop the run in flight, keeping everything it measured.
    Cancel,
    /// `U11`: dark and light.
    ToggleTheme,
    /// Re-run every environment probe, so a missing tool is recoverable
    /// without restarting.
    RecheckEnvironment,
    /// `U12`.
    TogglePalette,
    ClosePalette,
    /// `U9`: raising the tier is always a deliberate act, so this opens the
    /// dialog rather than changing anything.
    OpenTierDialog,
    /// Confirm the tier the dialog is offering.
    SetTier(binmap_core::config::TrustTier),
    CloseDialogs,
}

/// Which tab of the Findings Inspector is showing.
///
/// Lives here rather than in the view so an [`Action`] can name one without
/// the state layer depending on a view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InspectorTab {
    #[default]
    Hypothesis,
    Evidence,
    Proposal,
}

impl InspectorTab {
    pub fn label(self) -> &'static str {
        match self {
            InspectorTab::Hypothesis => "Hypothesis",
            InspectorTab::Evidence => "Evidence",
            InspectorTab::Proposal => "Proposal",
        }
    }

    pub const ALL: [InspectorTab; 3] =
        [InspectorTab::Hypothesis, InspectorTab::Evidence, InspectorTab::Proposal];
}

/// One entry in the command palette.
///
/// Every action the interface offers is reachable from here, which is the
/// whole of `U12`. An action that exists only as a click is a bug.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub label: String,
    pub group: &'static str,
    /// The shortcut, where it has one, as the palette prints it.
    pub shortcut: Option<&'static str>,
    pub action: Action,
}

impl AppState {
    /// Every command, filtered by `query`.
    ///
    /// Built from the live state rather than a fixed list, so a view the
    /// target cannot support is not offered — the same rule the nav rail
    /// follows, applied where a user is most likely to go looking.
    pub fn commands(&self, query: &str) -> Vec<Command> {
        let mut commands = Vec::new();

        for entry in self.nav_entries() {
            commands.push(Command {
                label: format!("Go to {}", entry.view.label()),
                group: "Views",
                shortcut: None,
                action: Action::SelectView(entry.view),
            });
        }

        commands.push(Command {
            label: "Sweep build configurations".into(),
            group: "Analysis",
            shortcut: Some("⌘⇧T"),
            action: Action::StartSweep,
        });
        if self.is_busy() {
            commands.push(Command {
                label: "Cancel the run, keeping what it measured".into(),
                group: "Analysis",
                shortcut: Some("Esc"),
                action: Action::Cancel,
            });
        }
        commands.push(Command {
            label: "Re-check the environment".into(),
            group: "Analysis",
            shortcut: None,
            action: Action::RecheckEnvironment,
        });

        for target in self.targets() {
            commands.push(Command {
                label: format!("Select target {}", target.id),
                group: "Targets",
                shortcut: None,
                action: Action::SelectTarget(target.id.clone()),
            });
        }

        commands.push(Command {
            label: "Change the trust tier".into(),
            group: "Session",
            shortcut: None,
            action: Action::OpenTierDialog,
        });
        commands.push(Command {
            label: "Switch between the dark and light themes".into(),
            group: "Session",
            shortcut: None,
            action: Action::ToggleTheme,
        });

        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return commands;
        }
        commands
            .into_iter()
            .filter(|command| command.label.to_lowercase().contains(&needle))
            .collect()
    }
}
