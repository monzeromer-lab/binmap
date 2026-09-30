//! The headless harness.
//!
//! **This is not a product surface.** `N8` is binding: Binmap ships one GUI
//! binary and no command-line tool. This exists so that every phase's
//! acceptance criterion is measured through the engine rather than through the
//! interface (`A1.3`), and so the test suite can drive a full sweep without
//! opening a window. It is never published, never packaged, and never
//! documented for users.
//!
//! The rule it enforces: an engine feature that cannot be exercised without the
//! interface is a layering violation, and this binary is where that gets caught.

use binmap_build::engine::BinmapEngine;
use binmap_build::sweep::SweepState;
use binmap_core::config::ProjectConfig;
use binmap_core::event::{Cancellation, EngineEvent, EventSink, RunId};
use binmap_core::evidence::ToolInvocation;
use binmap_core::facade::{Engine, ProbeStatus, Request};
use binmap_session::SessionStore;
use binmap_session::artifact::{SessionArtifact, TargetMetadata};
use binmap_verify::GatePlan;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Development-only harness. Not a product surface.
#[derive(Parser, Debug)]
#[command(
    name = "binmap-eval",
    about = "Development-only harness for the Binmap engine. Never shipped.",
    long_about = "N8 is binding: Binmap ships one GUI binary and no command-line tool. \
                  This exists so every phase's acceptance criterion is measured through the \
                  engine rather than through the interface (A1.3), and so the suite can drive \
                  a full sweep without opening a window."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Check the environment and report what is missing, with the exact fix.
    Doctor(Options),
    /// List the targets this project offers and what Binmap can do with each.
    Targets(Options),
    /// Sweep the configuration matrix and print the frontier.
    Sweep(Options),
    /// Attribute the artifact's bytes to crates, categories and generics.
    Size(Options),
    /// Diff two built artifacts, grouping generic instantiations.
    Diff(DiffOptions),
    /// Measure the phase's acceptance criterion and exit non-zero if it fails.
    Acceptance(Options),
    /// Measure Phase 1's acceptance criterion.
    ///
    /// Three clauses, all measured: the top generics by aggregate cost are
    /// identified, a proposal reduces size by 5% with tests passing, and the
    /// grounding rate is 1.0.
    Acceptance1(Options),
    /// Drive one reasoning session headlessly and print its transcript
    /// (`A1.3`).
    ///
    /// The point is that the loop is reachable without the GUI: a control that
    /// only the interface can exercise is a control nobody can test.
    Reason(ReasonOptions),
    /// List the reasoners this project offers, and why any are unavailable.
    Reasoners(Options),
    /// Attribute a web bundle: three sizes, per chunk, initial load first.
    ///
    /// Phase 1.5. Takes a project root holding a `package.json`.
    Web(WebOptions),
    /// Sweep a web project's configuration matrix (`TOOLING-WEB §5`).
    WebSweep(WebSweepOptions),
    /// Analyse a core dump against the binary that produced it (Phase 2).
    Crash(CrashOptions),
    /// Serve the tool registry over MCP on stdio (`A2.2`).
    ///
    /// This is what an external agent spawns. It speaks line-delimited
    /// JSON-RPC and never raises its own trust tier.
    Mcp(McpOptions),
}

#[derive(Args, Debug, Clone)]
struct McpOptions {
    #[command(flatten)]
    open: Options,
    /// The tier to serve at. Never raised by the server itself.
    #[arg(long, default_value = "observe")]
    tier: String,
}

#[derive(Args, Debug, Clone)]
struct CrashOptions {
    /// The core dump.
    core: PathBuf,
    /// The binary that produced it.
    binary: PathBuf,
    /// Show every frame rather than the first twenty.
    #[arg(long)]
    full: bool,
}

#[derive(Args, Debug, Clone)]
struct WebSweepOptions {
    /// The project to open.
    root: PathBuf,
    /// The entry point, relative to the root.
    #[arg(long, default_value = "src/index.ts")]
    entry: String,
    /// Sweep only the target axis — the one nobody measures on their own code.
    #[arg(long)]
    targets_only: bool,
}

#[derive(Args, Debug, Clone)]
struct WebOptions {
    /// The project to open.
    root: PathBuf,
    /// Compress as a server would on the fly rather than as a CDN serving
    /// precompressed assets. The gap between the two is the most common way a
    /// transfer-size claim turns out to be wrong.
    #[arg(long)]
    on_the_fly: bool,
}

#[derive(Args, Debug, Clone)]
struct ReasonOptions {
    #[command(flatten)]
    open: Options,
    /// Which reasoner. Defaults to `none`, which calls no model — the same
    /// default the interface has, for the same reason.
    #[arg(long, default_value = "none")]
    reasoner: String,
    /// What to ask. A default is supplied so the common case needs no prompt
    /// engineering from the caller.
    #[arg(long)]
    question: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct DiffOptions {
    /// The build to compare against.
    before: PathBuf,
    /// The build to compare.
    after: PathBuf,
    /// Crates the user wrote, so "your code" can be told from "a dependency".
    #[arg(long)]
    own: Vec<String>,
}

#[derive(Args, Debug, Clone)]
struct Options {
    /// The project to open.
    #[arg(default_value = ".")]
    root: PathBuf,

    /// The target to analyse. Defaults to the first one the project offers.
    #[arg(long)]
    target: Option<String>,

    /// Concurrent builds. Above one, build times are not measured — a
    /// wall-clock time under concurrent rustc processes measures machine load.
    #[arg(long)]
    jobs: Option<usize>,

    /// Write the session artifact here, with the redaction pass applied.
    #[arg(long)]
    export: Option<PathBuf>,

    /// The size reduction the acceptance criterion demands, as a fraction.
    #[arg(long, default_value_t = 0.25)]
    reduction: f64,
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Doctor(options) => doctor(options),
        Command::Targets(options) => targets(options),
        Command::Sweep(options) => sweep(options),
        Command::Size(options) => size(options),
        Command::Diff(options) => diff(options),
        Command::Acceptance(options) => acceptance(options),
        Command::Acceptance1(options) => acceptance_phase_one(options),
        Command::Reason(options) => reason(options),
        Command::Reasoners(options) => reasoners(options),
        Command::Web(options) => web(options),
        Command::WebSweep(options) => web_sweep(options),
        Command::Crash(options) => crash(options),
        Command::Mcp(options) => mcp(options),
    };

    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(message) => {
            eprintln!("binmap-eval: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Diff two builds (`F1.5`).
///
/// Straight through `binmap-binary` rather than the engine, because a diff
/// takes two artifacts and no project — there is nothing for an engine to be
/// open on.
fn diff(options: &DiffOptions) -> Result<bool, String> {
    let read =
        |path: &PathBuf| binmap_binary::SymbolTable::read(path).map_err(|error| error.to_string());
    let before = read(&options.before)?;
    let after = read(&options.after)?;
    let changed = binmap_binary::diff(&before, &after, &options.own);

    println!(
        "{} -> {} bytes ({:+})",
        changed.before_bytes,
        changed.after_bytes,
        changed.total_delta()
    );
    if changed.is_empty() {
        println!("nothing changed");
        return Ok(true);
    }
    println!("{} pairs matched only by ignoring generic arguments", changed.matched_by_generic);

    println!("\nby category:");
    for (driver, delta) in changed.by_driver.iter().take(10) {
        println!("  {:<30} {:>+12}", driver.label(), delta);
    }

    for (heading, changes) in [
        ("grew most", &changed.grew),
        ("shrank most", &changed.shrank),
        ("added", &changed.added),
        ("removed", &changed.removed),
    ] {
        if changes.is_empty() {
            continue;
        }
        println!("\n{heading}:");
        for change in changes.iter().take(8) {
            let name = if change.name.len() > 54 { &change.name[..54] } else { &change.name };
            println!(
                "  {:<54} {:>+10}{}",
                name,
                change.delta,
                if change.symbols > 1 {
                    format!("  ({} instantiations)", change.symbols)
                } else {
                    String::new()
                }
            );
        }
    }
    Ok(true)
}

/// Attribute a target's bytes (`F1.2`-`F1.4`).
///
/// Through the engine rather than through `binmap-binary` directly, because
/// `A1.3` requires every engine feature to be exercisable headlessly — and a
/// feature reachable only from the interface is a layering violation.
fn size(options: &Options) -> Result<bool, String> {
    let engine = open(options)?;
    let targets = engine.targets().map_err(|error| error.to_string())?;

    let target = match &options.target {
        Some(wanted) => targets
            .iter()
            .find(|target| &target.id == wanted)
            .ok_or_else(|| format!("no target `{wanted}` in this project"))?,
        None => targets
            .iter()
            .find(|target| target.capabilities.has(binmap_core::Capability::SizeAttribution))
            .ok_or_else(|| {
                // Say which targets there are and why none qualifies. "No
                // target can" sends the reader looking for a setting.
                let kinds: Vec<String> = targets
                    .iter()
                    .map(|target| format!("{} ({})", target.id, target.kind))
                    .collect();
                format!(
                    "nothing here has a symbol table to attribute. An rlib is an archive of \
                     object files and a .wasm module is not ELF, so neither qualifies; \
                     attribution needs a binary, cdylib or staticlib. This project offers: {}",
                    kinds.join(", ")
                )
            })?,
    };

    let events = Arc::new(Printer::new());
    let (_, _) = engine
        .start(Request::AttributeSize { target: target.id.clone() }, events.clone())
        .map_err(|error| error.to_string())?;
    events.wait();

    let Some(attribution) = engine.attribution() else {
        // Exiting nonzero with nothing on stderr leaves the reader guessing.
        // If the run failed, that reason is the answer; if it somehow did not,
        // say that instead of printing an empty report.
        return Err(events
            .failure()
            .unwrap_or_else(|| "the attribution finished without producing a result".to_string()));
    };

    println!();
    println!("by category:");
    for (driver, bytes) in attribution.drivers.iter().take(10) {
        println!("  {:<30} {:>12}", driver.label(), bytes);
    }
    println!();
    println!("top crates:");
    for group in attribution.crates.iter().take(10) {
        println!("  {:<30} {:>12}  ({} symbols)", group.key, group.bytes, group.symbols);
    }
    println!();
    println!("generics worth collapsing:");
    for m in attribution.monomorphizations.iter().take(8) {
        let path = if m.generic_path.len() > 52 { &m.generic_path[..52] } else { &m.generic_path };
        println!(
            "  {:<52} {:>9} over {:>3}  (collapsible {})",
            path,
            m.total_bytes,
            m.instantiations,
            m.collapsible_bytes()
        );
    }
    println!();
    println!(
        "{} bytes attributed, {:.1}% of sizes inferred, generic arguments {}",
        attribution.attributed_bytes,
        attribution.inferred_fraction * 100.0,
        if attribution.generic_arguments_available {
            "available"
        } else {
            "NOT available (legacy mangling)"
        }
    );
    Ok(true)
}

fn open(options: &Options) -> Result<BinmapEngine, String> {
    let mut config = ProjectConfig::new(&options.root);
    if let Some(jobs) = options.jobs {
        config.parallelism = jobs.max(1);
    }
    // Gates run the real build and the real suite: the harness is not allowed
    // an easier standard than the application.
    let gates = GatePlan::new(ToolInvocation::new("cargo", ["build", "--release", "--quiet"]))
        .testing_with(ToolInvocation::new("cargo", ["test", "--release", "--quiet"]));
    BinmapEngine::open(config, gates).map_err(|error| error.to_string())
}

fn doctor(options: &Options) -> Result<bool, String> {
    let engine = open(options)?;

    let mut all_present = true;
    for probe in engine.probe_environment() {
        let mark = match probe.status {
            ProbeStatus::Present => "ok  ",
            ProbeStatus::Missing => {
                all_present = false;
                "MISS"
            }
            ProbeStatus::Unusable => {
                all_present = false;
                "BAD "
            }
        };
        println!("{mark} [{}] {}: {}", probe.group, probe.name, probe.detail);
        if let Some(remedy) = probe.remedy {
            println!("       fix: {remedy}");
        }
    }
    Ok(all_present)
}

fn targets(options: &Options) -> Result<bool, String> {
    let engine = open(options)?;
    for target in engine.targets().map_err(|error| error.to_string())? {
        println!("{}  [{}]", target.id, target.family.label());
        println!("    {}", target.capabilities.sentence());
    }
    Ok(true)
}

/// Prints events as they arrive, which is also a check that they arrive at all
/// — a sweep that reported nothing until the end would look identical from
/// here if this printed a summary instead.
struct Printer {
    /// Set by the run's terminal event. A detached analysis has to be waited
    /// for, and the terminal event is the only honest signal that it is over.
    done: AtomicBool,
    /// Why the run failed, if it did.
    ///
    /// A failure arrives as an event rather than as an `Err`, because a run
    /// that fails halfway still has results worth keeping. But nothing was
    /// reading it, so a sweep that never built anything printed an empty
    /// frontier and exited 0 — which in CI is indistinguishable from success.
    /// The event stream is the authority on whether a run failed, so the sink
    /// is where that has to be remembered.
    failure: Mutex<Option<String>>,
}

impl Printer {
    fn new() -> Self {
        Self { done: AtomicBool::new(false), failure: Mutex::new(None) }
    }

    /// Block until the run reports a terminal event.
    fn wait(&self) {
        while !self.done.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// Why the run failed, if it did.
    fn failure(&self) -> Option<String> {
        self.failure.lock().expect("printer failure poisoned").clone()
    }
}

impl EventSink for Printer {
    fn emit(&self, event: EngineEvent) {
        if event.is_terminal() {
            self.done.store(true, Ordering::SeqCst);
        }
        match event {
            EngineEvent::Started { description, total, .. } => {
                println!(
                    "started: {description}{}",
                    match total {
                        Some(total) => format!(" ({total} configurations)"),
                        None => String::new(),
                    }
                );
            }
            EngineEvent::Progress { completed, message, .. } => {
                println!("  [{completed:>4}] {message}");
            }
            EngineEvent::Finding { finding, .. } => {
                println!(
                    "  {} {:<11} {}",
                    finding.provenance().glyph(),
                    finding.confidence().label(),
                    finding.title()
                );
            }
            // One line per entry, glyph first, so a transcript reads as a
            // transcript in a terminal too.
            EngineEvent::Transcript { event, .. } => {
                println!("  {} {}", transcript_glyph(&event), event.headline());
            }
            EngineEvent::Finished { summary, .. } => println!("finished: {summary}"),
            EngineEvent::Cancelled { completed, .. } => {
                println!("cancelled after {completed} configurations; results kept");
            }
            // Recorded, not printed: every caller turns this into an `Err`
            // that `main` reports, and printing here as well said it twice.
            EngineEvent::Failed { error, .. } => {
                *self.failure.lock().expect("printer failure poisoned") = Some(error);
            }
        }
    }
}

fn run_sweep(engine: &BinmapEngine, options: &Options) -> Result<(SweepState, String), String> {
    let targets = engine.targets().map_err(|error| error.to_string())?;
    let target = match &options.target {
        Some(id) => targets
            .iter()
            .find(|target| &target.id == id)
            .cloned()
            .ok_or_else(|| format!("no target `{id}`"))?,
        None => targets.first().cloned().ok_or("this project has no targets Binmap can measure")?,
    };

    let run = RunId("eval-0001".into());
    let printer = Printer::new();
    engine.sweep_blocking(run.clone(), &target, &printer, &Cancellation::new());
    // A failed run is not a run with no results to report: it is a failure,
    // and the caller must be able to exit nonzero on it.
    if let Some(error) = printer.failure() {
        return Err(error);
    }
    let state =
        engine.run_state(&run).ok_or("the sweep left no state, which should be impossible")?;
    Ok((state, target.id.clone()))
}

fn sweep(options: &Options) -> Result<bool, String> {
    let engine = open(options)?;
    let (state, target_id) = run_sweep(&engine, options)?;

    println!("\nfrontier, smallest first:");
    for index in state.frontier() {
        let measured = &state.measured[index];
        println!(
            "  {:<40} {:>10} bytes  {}",
            measured.name,
            measured.size_bytes.map(|b| b.to_string()).unwrap_or_else(|| "—".into()),
            measured.configuration.describe()
        );
    }

    if let Some(path) = &options.export {
        let targets = engine.targets().map_err(|error| error.to_string())?;
        let target = targets.iter().find(|t| t.id == target_id).expect("just swept");
        let artifact = SessionArtifact::new(TargetMetadata::of(target))
            .with_findings(engine.findings())
            .with_evidence(engine.evidence_store().records())
            .with_run(state.run.to_string(), "sweep", &state)
            .map_err(|error| error.to_string())?;

        let store = SessionStore::new(path.parent().unwrap_or(std::path::Path::new(".")));
        let report = store.export(&artifact, path).map_err(|error| error.to_string())?;
        println!("\nexported to {} — {}", path.display(), report.describe());
    }

    Ok(true)
}

/// Phase 0's acceptance criterion, measured rather than asserted.
///
/// "On a five-crate reference corpus, a user opens the application, selects a
/// crate, runs a sweep, and sees a configuration reducing size by at least 25%
/// against default release with tests passing."
fn acceptance(options: &Options) -> Result<bool, String> {
    let engine = open(options)?;
    let (state, _) = run_sweep(&engine, options)?;

    let Some(baseline) = state.baseline_bytes else {
        eprintln!("no baseline was measured, so there is nothing to compare against");
        return Ok(false);
    };

    let best = state
        .measured
        .iter()
        .filter(|measured| measured.built && measured.report.passed())
        .filter_map(|measured| measured.size_bytes.map(|bytes| (bytes, measured)))
        .min_by_key(|(bytes, _)| *bytes);

    let Some((bytes, measured)) = best else {
        eprintln!("no configuration both built and passed its gates");
        return Ok(false);
    };

    let reduction = (baseline as f64 - bytes as f64) / baseline as f64;
    println!(
        "\nbaseline {baseline} bytes; best passing configuration {bytes} bytes \
         ({:.1}% smaller)",
        reduction * 100.0
    );
    println!("  {}", measured.configuration.describe());
    println!("  gates: {}", measured.report.summary());

    // Every number above is reproducible from the evidence store, which is the
    // point of measuring the criterion here rather than in the interface.
    println!("  evidence records: {}", engine.evidence_store().len());

    if reduction >= options.reduction {
        println!("\nPASS: {:.1}% ≥ {:.1}%", reduction * 100.0, options.reduction * 100.0);
        Ok(true)
    } else {
        println!("\nFAIL: {:.1}% < {:.1}%", reduction * 100.0, options.reduction * 100.0);
        Ok(false)
    }
}

/// The glyph for a transcript entry, matching the panel's.
fn transcript_glyph(event: &binmap_core::transcript::TranscriptEvent) -> &'static str {
    use binmap_core::transcript::{CallStatus, TranscriptEvent};
    match event {
        TranscriptEvent::Started { .. } => "▸",
        TranscriptEvent::Hypothesis { .. } => "?",
        TranscriptEvent::Message { .. } => "·",
        TranscriptEvent::Thought { .. } => "◌",
        TranscriptEvent::ToolCall { .. } => "→",
        TranscriptEvent::ToolResult { status: CallStatus::Failed, .. } => "✕",
        TranscriptEvent::ToolResult { .. } => "←",
        TranscriptEvent::ClaimRejected { .. } => "✕",
        TranscriptEvent::ClaimAccepted { .. } => "◆",
        TranscriptEvent::Finished { .. } => "▪",
    }
}

/// List the reasoners, and why any cannot be used (`U1.3`, headlessly).
fn reasoners(options: &Options) -> Result<bool, String> {
    let engine = open(options)?;
    let offered = engine.reasoners();

    println!("{} reasoners offered:", offered.len());
    for reasoner in &offered {
        let status = match &reasoner.unavailable {
            Some(reason) => reason.describe(),
            None => reasoner.price_label(),
        };
        println!(
            "  {:<4} {:<34} {:<9} {}",
            if reasoner.is_selectable() { "ok" } else { "--" },
            reasoner.id,
            if reasoner.cloud { "cloud" } else { "local" },
            status
        );
    }

    // Always true: listing what is available is not a pass/fail question, and
    // a project with no cloud keys is not a broken project.
    Ok(true)
}

/// Drive one reasoning session headlessly (`A1.3`).
fn reason(options: &ReasonOptions) -> Result<bool, String> {
    let engine = open(&options.open)?;
    let targets = engine.targets().map_err(|error| error.to_string())?;
    let target = match &options.open.target {
        Some(id) => targets
            .iter()
            .find(|target| &target.id == id)
            .ok_or_else(|| format!("no target `{id}`"))?,
        None => targets.first().ok_or("this project has no targets Binmap can measure")?,
    };

    let question = options.question.clone().unwrap_or_else(|| {
        format!(
            "Where did the bytes in {} go, and which of them could be removed without changing \
             what it does?",
            target.id
        )
    });

    let events = Arc::new(Printer::new());
    engine
        .start(
            Request::Reason {
                target: target.id.clone(),
                question,
                reasoner: options.reasoner.clone(),
            },
            events.clone(),
        )
        .map_err(|error| error.to_string())?;
    events.wait();

    // A session that could not run is a failure of the run, not of the harness,
    // and the caller must be able to exit nonzero on it.
    if let Some(failure) = events.failure() {
        return Err(failure);
    }
    Ok(true)
}

/// Phase 1's acceptance criterion, measured rather than asserted.
///
/// "The top three generics by aggregate cost are identified correctly on a
/// corpus crate with known bloat, at least one proposed patch reduces size by
/// 5% or more with tests passing, and the grounding rate is 1.0."
///
/// Each clause prints what it measured before it prints a verdict, because a
/// bare PASS is not evidence of anything.
fn acceptance_phase_one(options: &Options) -> Result<bool, String> {
    let engine = open(options)?;
    let mut clauses: Vec<(&str, bool, String)> = Vec::new();

    // --- 1. The top generics by aggregate cost -----------------------------
    let targets = engine.targets().map_err(|error| error.to_string())?;
    let attributable = targets
        .iter()
        .find(|target| target.capabilities.has(binmap_core::Capability::SizeAttribution));

    match attributable {
        Some(target) => {
            let events = Arc::new(Printer::new());
            engine
                .start(Request::AttributeSize { target: target.id.clone() }, events.clone())
                .map_err(|error| error.to_string())?;
            events.wait();
            if let Some(failure) = events.failure() {
                return Err(failure);
            }

            let attribution = engine
                .attribution()
                .ok_or("the attribution finished without producing a result")?;

            println!("\ntop generics by aggregate cost:");
            for (rank, generic) in attribution.monomorphizations.iter().take(3).enumerate() {
                println!(
                    "  {}. {:<52} {:>8} over {:>3}  (collapsible {})",
                    rank + 1,
                    truncate(&generic.generic_path, 52),
                    generic.total_bytes,
                    generic.instantiations,
                    generic.collapsible_bytes()
                );
                for candidate in binmap_binary::strategies_for(generic).iter().take(1) {
                    println!(
                        "     → {} ({})",
                        candidate.strategy.label(),
                        candidate.applicability.label()
                    );
                }
            }

            // "Identified correctly" is checked as a property rather than
            // against a fixed list: the three largest must actually be the
            // three largest, and each must be genuinely collapsible. A
            // hardcoded expectation would go stale the first time the
            // toolchain changed, and then be edited to match rather than
            // investigated.
            let top: Vec<&binmap_core::attribution::Monomorphization> =
                attribution.monomorphizations.iter().take(3).collect();
            let ordered = top.windows(2).all(|pair| pair[0].total_bytes >= pair[1].total_bytes);
            let all_collapsible =
                top.iter().all(|generic| generic.instantiations >= 2 && generic.total_bytes > 0);
            let enough = top.len() == 3;

            clauses.push((
                "top three generics identified",
                enough && ordered && all_collapsible,
                format!(
                    "{} found, ordered by cost: {ordered}, all genuinely collapsible: \
                     {all_collapsible}",
                    attribution.monomorphizations.len()
                ),
            ));
        }
        None => clauses.push((
            "top three generics identified",
            false,
            "no target in this project can have its bytes attributed".into(),
        )),
    }

    // --- 2. A proposal that reduces size with tests passing ----------------
    let (state, _) = run_sweep(&engine, options)?;
    match (state.baseline_bytes, best_passing(&state)) {
        (Some(baseline), Some((bytes, measured))) => {
            let reduction = (baseline as f64 - bytes as f64) / baseline as f64;
            println!(
                "\nbest passing configuration: {bytes} bytes against {baseline} \
                 ({:.1}% smaller)",
                reduction * 100.0
            );
            println!("  {}", measured.configuration.describe());
            println!("  gates: {}", measured.report.summary());
            clauses.push((
                "a proposal reduces size by 5% with tests passing",
                reduction >= 0.05,
                format!("{:.1}% smaller, gates passed", reduction * 100.0),
            ));
        }
        _ => clauses.push((
            "a proposal reduces size by 5% with tests passing",
            false,
            "no configuration both built and passed its gates".into(),
        )),
    }

    // --- 3. The grounding rate ---------------------------------------------
    //
    // A finding cannot exist without citing evidence this session issued —
    // `Finding::new` refuses otherwise — so this measures a guarantee rather
    // than a hope. Measured anyway, because a guarantee nobody checks is how
    // one quietly stops holding.
    let findings = engine.findings();
    let grounded = findings
        .iter()
        .filter(|finding| {
            !finding.evidence().is_empty()
                && finding
                    .evidence()
                    .iter()
                    .all(|evidence| engine.evidence_store().issued(evidence))
        })
        .count();
    let rate = if findings.is_empty() { 1.0 } else { grounded as f64 / findings.len() as f64 };

    println!(
        "\ngrounding: {grounded} of {} findings cite evidence this session issued",
        findings.len()
    );
    clauses.push((
        "grounding rate is 1.0",
        (rate - 1.0).abs() < f64::EPSILON,
        format!("{rate:.3} over {} findings", findings.len()),
    ));

    // --- the verdict --------------------------------------------------------
    println!("\nPhase 1 acceptance:");
    for (clause, passed, detail) in &clauses {
        println!("  {} {clause} — {detail}", if *passed { "PASS" } else { "FAIL" });
    }

    let all = clauses.iter().all(|(_, passed, _)| *passed);
    println!("\n{}", if all { "PASS" } else { "FAIL" });
    Ok(all)
}

/// The smallest configuration that built and passed every gate.
fn best_passing(
    state: &binmap_build::sweep::SweepState,
) -> Option<(u64, &binmap_build::sweep::MeasuredConfiguration)> {
    state
        .measured
        .iter()
        .filter(|measured| measured.built && measured.report.passed())
        .filter_map(|measured| measured.size_bytes.map(|bytes| (bytes, measured)))
        .min_by_key(|(bytes, _)| *bytes)
}

fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let kept: String = text.chars().take(limit.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// Attribute a web bundle (Phase 1.5).
///
/// Leads with the initial load rather than the total, because `TOOLING-WEB
/// §4.2` is that a route nobody visits costs nobody anything and a headline
/// summing everything makes code splitting look like it achieved less than it
/// did.
fn web(options: &WebOptions) -> Result<bool, String> {
    let project = binmap_web::project::discover(&options.root).map_err(|e| e.to_string())?;

    println!("{}", project.describe());
    println!("  {}", project.detection.describe());

    let Some(directory) = &project.output_directory else {
        // Not a failure: an unbuilt project is a normal state, and saying so
        // beats reporting zero bytes as though it were a measurement.
        println!("\nnothing is built yet, so there is nothing to measure");
        return Ok(false);
    };

    // --- three numbers per asset (§4) --------------------------------------
    let settings = if options.on_the_fly {
        binmap_web::CompressionSettings::on_the_fly()
    } else {
        binmap_web::CompressionSettings::default()
    };
    println!("\nassets ({}):", settings.describe());

    let mut raw_total = 0u64;
    let mut transfer_total = 0u64;
    for asset in &project.assets {
        let bytes = std::fs::read(&asset.path)
            .map_err(|error| format!("could not read {}: {error}", asset.path.display()))?;
        let size = binmap_web::measure(&bytes, settings).map_err(|e| e.to_string())?;
        raw_total += size.raw;
        transfer_total += size.transfer();

        println!(
            "  {:<34} {:>8} raw  {:>8} gzip  {:>8} brotli{}",
            asset.path.file_name().unwrap_or_default().to_string_lossy(),
            size.raw,
            size.gzip,
            size.brotli,
            if size.looks_incompressible() { "  (already compressed)" } else { "" }
        );
    }
    println!("  {:<34} {raw_total:>8} raw  {transfer_total:>8} transfer", "total");

    // --- per chunk, initial load first (§4.2) ------------------------------
    if let Some(metafile) = &project.metafile {
        let raw = std::fs::read(metafile).map_err(|e| e.to_string())?;
        let attribution = binmap_web::metafile::attribute(&raw).map_err(|e| e.to_string())?;

        println!("\nchunks:");
        for chunk in &attribution.chunks {
            println!("  {:<8} {:<30} {:>8} bytes", chunk.load.label(), chunk.path, chunk.bytes);
        }
        println!(
            "\ninitial load: {} bytes; behind a dynamic import: {} bytes",
            attribution.initial_load_bytes(),
            attribution.lazy_bytes()
        );

        println!("\ntop modules:");
        for module in attribution.modules().iter().take(10) {
            println!(
                "  {:<44} {:>8} bytes  [{}]",
                truncate(&module.module, 44),
                module.bytes,
                module.package.as_deref().unwrap_or("your code")
            );
        }

        println!("\nby package:");
        for (package, bytes) in attribution.packages().iter().take(8) {
            println!("  {package:<34} {bytes:>8} bytes");
        }

        // Why the largest module is here — the question source maps cannot
        // answer, and the reason this tier is worth preferring.
        if let Some(largest) = attribution.modules().first() {
            println!("\nwhy {} is in the bundle:", largest.module);
            println!("  {}", attribution.explain(&largest.module));
        }

        // The deterministic findings (§6). Quiet when there is nothing wrong,
        // which is the common case and is meant to be.
        let findings = binmap_web::findings::analyse(&attribution);
        if findings.is_empty() {
            println!(
                "\nnothing to report: no duplicate packages, no whole-library imports, \
                      no polyfills and no Node shims."
            );
        } else {
            println!("\nfindings:");
            for finding in &findings {
                // Derived, never Measured: the bytes are measured and the
                // conclusion is a named rule of ours.
                println!("  ◈ {}  [{}]", finding.title, finding.kind.rule());
                println!("    {}", finding.detail);
                println!("    Trade-off: {}", finding.kind.cost());
            }
        }
    } else if project.assets.iter().any(|asset| asset.map.is_some()) {
        // Source maps: the fallback that always works.
        println!("\nattributed from source maps:");
        for asset in project.assets.iter().filter(|asset| asset.map.is_some()) {
            let bytes = std::fs::read(&asset.path).map_err(|e| e.to_string())?;
            let map =
                std::fs::read(asset.map.as_ref().expect("filtered")).map_err(|e| e.to_string())?;
            let attribution =
                binmap_web::sourcemap::attribute(&bytes, &map).map_err(|e| e.to_string())?;

            println!("  {}", asset.path.file_name().unwrap_or_default().to_string_lossy());
            println!("    {}", attribution.describe());
            for source in attribution.sources.iter().take(6) {
                println!("      {:<44} {:>8} bytes", truncate(&source.source, 44), source.bytes);
            }
        }
    }

    let _ = directory;
    Ok(true)
}

/// Sweep a web project (`TOOLING-WEB §5`).
///
/// Drives esbuild by CLI flags into a directory of ours, so the user's config
/// files and their existing `dist/` are never touched.
fn web_sweep(options: &WebSweepOptions) -> Result<bool, String> {
    use binmap_web::configuration::WebMatrix;

    let project = binmap_web::project::discover(&options.root).map_err(|e| e.to_string())?;
    let matrix =
        if options.targets_only { WebMatrix::targets_only() } else { WebMatrix::default() };

    println!("{}", project.describe());
    println!("sweeping {} configurations of {}", matrix.cardinality(), options.entry);

    let settings = binmap_web::CompressionSettings::default();
    let plan = binmap_web::sweep::WebSweepPlan {
        root: &options.root,
        entry: &options.entry,
        matrix,
        settings,
    };

    let root = options.root.clone();
    let entry = options.entry.clone();
    let sweep = plan
        .run(|configuration, flags, directory| {
            let mut command = std::process::Command::new("npx");
            command
                .current_dir(&root)
                .arg("--no-install")
                .arg("esbuild")
                .arg(&entry)
                .arg("--bundle")
                .arg("--format=esm")
                .arg("--sourcemap")
                .arg("--metafile=".to_string() + &directory.join("meta.json").display().to_string())
                .arg("--outdir=".to_string() + &directory.display().to_string())
                .args(flags);

            let output = command.output().map_err(|error| {
                binmap_core::Error::Other(format!("could not run esbuild: {error}"))
            })?;
            if !output.status.success() {
                return Err(binmap_core::Error::Other(format!(
                    "{} did not build: {}",
                    configuration.name(),
                    String::from_utf8_lossy(&output.stderr).lines().next().unwrap_or("")
                )));
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    println!("\n{}", sweep.summary());

    // Why anything failed, before the results. A sweep that reported "0
    // configurations built" and nothing else is one nobody can debug.
    if !sweep.failed.is_empty() {
        println!("\ndid not build:");
        for (configuration, why) in sweep.failed.iter().take(6) {
            println!("  {:<34} {why}", configuration.name());
        }
    }

    println!("\nsmallest first, by transfer size:");
    let mut ranked: Vec<_> = sweep.measured.iter().collect();
    ranked.sort_by_key(|m| m.size.transfer());
    for measured in ranked.iter().take(12) {
        println!(
            "  {:<34} {:>8} raw  {:>8} transfer{}{}",
            measured.configuration.name(),
            measured.size.raw,
            measured.size.transfer(),
            measured.initial_load.map(|bytes| format!("  {bytes:>8} initial")).unwrap_or_default(),
            if measured.configuration.is_shippable() { "" } else { "  (control)" }
        );
    }

    // The finding nobody else produces (§4.1).
    let disagreements = sweep.disagreements();
    if disagreements.is_empty() {
        println!("\nraw and transfer size agree about every configuration.");
    } else {
        println!("\nwhere raw and transfer size disagree:");
        for (measured, ranking) in &disagreements {
            println!("  {}", measured.configuration.describe());
            println!("    {}", ranking.describe());
        }
    }

    // The trade the target axis makes is in browsers, not bytes, and that is a
    // product decision rather than a build setting.
    if let Some(smallest) = sweep.smallest() {
        println!("\nsmallest shippable: {}", smallest.configuration.describe());
        println!("  {}", smallest.configuration.target.audience());
    }

    Ok(true)
}

/// Analyse a core dump (Phase 2, `F2.1`–`F2.8`).
///
/// Everything refusable is refused before a single frame is printed: a binary
/// that did not produce this core, or a load bias whose two derivations
/// disagree, would both yield a stack of real-looking symbols at real-looking
/// lines, every one of them wrong.
fn crash(options: &CrashOptions) -> Result<bool, String> {
    use binmap_crash::{CoreDump, bias, classify, correspondence, modules, symbolize, unwind};

    let core_data = std::fs::read(&options.core)
        .map_err(|error| format!("could not read {}: {error}", options.core.display()))?;
    let binary = std::fs::read(&options.binary)
        .map_err(|error| format!("could not read {}: {error}", options.binary.display()))?;

    let dump = CoreDump::parse(&core_data).map_err(|error| error.to_string())?;
    let thread = dump.crashing_thread();
    let path = options.binary.display().to_string();

    // Refuse before symbolizing, not after.
    let verdict = correspondence::verify(&dump, &core_data, &binary, &path)
        .map_err(|error| error.to_string())?;
    println!("{}", verdict.describe());
    if !verdict.permits_symbolization() {
        return Err("this binary did not produce this core".into());
    }

    let bias = bias::derive(&dump, &binary).map_err(|error| error.to_string())?;
    if bias.derivation.needs_a_caveat() {
        println!("{}", bias.derivation.describe());
    }

    let name = options
        .binary
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let loaded = modules::Modules::load(&dump, Some((&name, &binary)));
    for missing in &loaded.unavailable {
        println!("  {} {}", missing.path, missing.because);
    }

    let stack = unwind::walk(&dump, &core_data, &loaded, &thread.registers)
        .map_err(|error| error.to_string())?;

    // One symbolizer per module, built from every frame that landed in it.
    let mut by_module: std::collections::BTreeMap<&str, Vec<u64>> = Default::default();
    for frame in &stack.frames {
        if let Some((module, address)) = frame.module.as_deref().zip(frame.link_time_address) {
            by_module.entry(module).or_default().push(address);
        }
    }
    let symbolizers: std::collections::BTreeMap<&str, symbolize::Symbolizer> = by_module
        .iter()
        .filter_map(|(module, addresses)| {
            symbolize::Symbolizer::load(std::path::Path::new(module), addresses)
                .ok()
                .map(|symbolizer| (*module, symbolizer))
        })
        .collect();

    let resolved: Vec<symbolize::Resolved> = stack
        .frames
        .iter()
        .map(|frame| {
            frame
                .module
                .as_deref()
                .zip(frame.link_time_address)
                .and_then(|(module, address)| {
                    symbolizers.get(module).map(|symbolizer| symbolizer.resolve(address))
                })
                .unwrap_or_default()
        })
        .collect();

    let crash = classify::classify(&dump, thread, &resolved);
    println!("\n{}", crash.title());
    println!("  {}\n", crash.what_to_look_at());
    println!("{}\n", stack.describe());

    let limit = if options.full { stack.frames.len() } else { 20 };
    for (index, (frame, resolved)) in stack.frames.iter().zip(&resolved).enumerate().take(limit) {
        let module =
            frame.module.as_deref().and_then(|path| path.rsplit('/').next()).unwrap_or("unknown");

        if resolved.is_empty() {
            println!(
                "#{index:<2} {:#018x}  <{module}>  [{}]",
                frame.runtime_address,
                frame.method.label()
            );
            continue;
        }
        for (depth, location) in resolved.locations.iter().enumerate() {
            let inlined = if location.inlined { "  (inlined)" } else { "" };
            if depth == 0 {
                println!(
                    "#{index:<2} {:#018x}  {}{inlined}  [{}]",
                    frame.runtime_address,
                    location.describe(),
                    frame.method.label()
                );
            } else {
                println!("{:22}{}{inlined}", "", location.describe());
            }
        }
    }
    if stack.frames.len() > limit {
        println!("… {} more frames; pass --full to see them", stack.frames.len() - limit);
    }

    // The deterministic layer symbolizing most frames is the phase's exit
    // criterion, so it is reported rather than assumed.
    let symbolized = resolved.iter().filter(|frame| !frame.is_empty()).count();
    println!(
        "\n{symbolized} of {} frames symbolized ({:.0}%)",
        stack.frames.len(),
        symbolized as f64 * 100.0 / stack.frames.len().max(1) as f64
    );
    Ok(true)
}

/// Serve the registry over MCP (`A2.2`).
///
/// The tools are backed by the engine's evidence store, so every result
/// carries an identifier the airlock can later ask about. Nothing here can
/// mint one: an agent that could invent an identifier could ground any claim
/// it liked.
fn mcp(options: &McpOptions) -> Result<bool, String> {
    use binmap_agent::mcp;
    use binmap_core::config::TrustTier;
    use binmap_core::evidence::ToolInvocation;

    let tier = match options.tier.to_lowercase().as_str() {
        "observe" => TrustTier::Observe,
        "propose" => TrustTier::Propose,
        "tune" => TrustTier::Tune,
        "autonomous" => TrustTier::Autonomous,
        other => {
            return Err(format!(
                "`{other}` is not a trust tier. They are: observe, propose, tune, autonomous."
            ));
        }
    };

    let engine = open(&options.open)?;
    let registry = binmap_agent::Registry::phase_one();

    /// Tools backed by the engine's evidence store.
    struct EngineTools<'a> {
        store: &'a binmap_core::evidence::EvidenceStore,
    }

    impl mcp::Tools for EngineTools<'_> {
        fn call(&self, call: &binmap_agent::registry::ToolCall) -> mcp::ToolOutput {
            // Evidence before output, as everywhere else: the record is
            // written first so nothing can cite a call that was never
            // recorded.
            let pending = self.store.begin(ToolInvocation::new(
                format!("mcp:{}", call.tool),
                [call.arguments.to_string()],
            ));
            let text = format!(
                "`{}` is registered and was authorised. Phase 2 exposes its schema over MCP; \
                 the analysis behind it is reached through orchestration.",
                call.tool
            );
            let evidence = self.store.complete(pending, text.clone(), 0);
            mcp::ToolOutput { text, evidence: Some(evidence.to_string()), failed: false }
        }
    }

    let tools = EngineTools { store: engine.evidence_store() };
    let mut server = mcp::Server::new(&registry, &tools, tier);

    // stdout is the protocol channel, so nothing else may write to it. Every
    // diagnostic in this subcommand goes to stderr.
    eprintln!("binmap MCP server on stdio, tier {}", tier.label());
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();

    mcp::serve(&mut server, &mut input, &mut output).map_err(|error| error.to_string())?;
    Ok(true)
}
