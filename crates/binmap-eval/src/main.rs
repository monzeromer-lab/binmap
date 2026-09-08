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
use binmap_core::facade::{Engine, ProbeStatus};
use binmap_session::artifact::{SessionArtifact, TargetMetadata};
use binmap_session::SessionStore;
use binmap_verify::GatePlan;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = arguments.first().map(String::as_str).unwrap_or("help");

    let result = match command {
        "probe" => probe(&arguments[1..]),
        "targets" => targets(&arguments[1..]),
        "sweep" => sweep(&arguments[1..]),
        "acceptance" => acceptance(&arguments[1..]),
        "help" | "--help" | "-h" => {
            usage();
            return ExitCode::SUCCESS;
        }
        other => Err(format!("unknown command `{other}`")),
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

fn usage() {
    eprintln!(
        "binmap-eval — development-only harness. Not a product surface.\n\
         \n\
         binmap-eval probe <project>\n\
         binmap-eval targets <project>\n\
         binmap-eval sweep <project> [--target ID] [--jobs N] [--export FILE]\n\
         binmap-eval acceptance <project> [--target ID] [--reduction 0.25]\n"
    );
}

/// Options every command shares.
struct Options {
    root: PathBuf,
    target: Option<String>,
    jobs: Option<usize>,
    export: Option<PathBuf>,
    reduction: f64,
}

fn parse(arguments: &[String]) -> Result<Options, String> {
    let root = arguments
        .first()
        .filter(|first| !first.starts_with("--"))
        .map(PathBuf::from)
        .ok_or("a project path is required")?;

    let mut options =
        Options { root, target: None, jobs: None, export: None, reduction: 0.25 };
    let mut rest = arguments[1..].iter();
    while let Some(flag) = rest.next() {
        let value = || rest.clone().next().cloned().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--target" => options.target = Some(value()?),
            "--jobs" => {
                options.jobs = Some(value()?.parse().map_err(|_| "--jobs needs a number")?)
            }
            "--export" => options.export = Some(PathBuf::from(value()?)),
            "--reduction" => {
                options.reduction =
                    value()?.parse().map_err(|_| "--reduction needs a fraction, e.g. 0.25")?
            }
            other if other.starts_with("--") => return Err(format!("unknown flag `{other}`")),
            _ => continue,
        }
        // Skip the value we just consumed.
        rest.next();
    }
    Ok(options)
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

fn probe(arguments: &[String]) -> Result<bool, String> {
    let options = parse(arguments)?;
    let engine = open(&options)?;

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

fn targets(arguments: &[String]) -> Result<bool, String> {
    let options = parse(arguments)?;
    let engine = open(&options)?;
    for target in engine.targets().map_err(|error| error.to_string())? {
        println!("{}  [{}]", target.id, target.family.label());
        println!("    {}", target.capabilities.sentence());
    }
    Ok(true)
}

/// Prints events as they arrive, which is also a check that they arrive at all
/// — a sweep that reported nothing until the end would look identical from
/// here if this printed a summary instead.
struct Printer;

impl EventSink for Printer {
    fn emit(&self, event: EngineEvent) {
        match event {
            EngineEvent::Started { description, total, .. } => {
                println!("started: {description}{}", match total {
                    Some(total) => format!(" ({total} configurations)"),
                    None => String::new(),
                });
            }
            EngineEvent::Progress { completed, message, .. } => {
                println!("  [{completed:>4}] {message}");
            }
            EngineEvent::Finding { finding, .. } => {
                println!(
                    "  {} {:<11} {}",
                    finding.provenance.glyph(),
                    finding.confidence.label(),
                    finding.title
                );
            }
            EngineEvent::Finished { summary, .. } => println!("finished: {summary}"),
            EngineEvent::Cancelled { completed, .. } => {
                println!("cancelled after {completed} configurations; results kept");
            }
            EngineEvent::Failed { error, .. } => eprintln!("failed: {error}"),
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
        None => targets
            .first()
            .cloned()
            .ok_or("this project has no targets Binmap can measure")?,
    };

    let run = RunId("eval-0001".into());
    engine.sweep_blocking(run.clone(), &target, &Printer, &Cancellation::new());
    let state = engine
        .run_state(&run)
        .ok_or("the sweep left no state, which should be impossible")?;
    Ok((state, target.id.clone()))
}

fn sweep(arguments: &[String]) -> Result<bool, String> {
    let options = parse(arguments)?;
    let engine = open(&options)?;
    let (state, target_id) = run_sweep(&engine, &options)?;

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
fn acceptance(arguments: &[String]) -> Result<bool, String> {
    let options = parse(arguments)?;
    let engine = open(&options)?;
    let (state, _) = run_sweep(&engine, &options)?;

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
