//! Sweeping the matrix: build, measure, gate, and derive the frontier.
//!
//! Three ordering decisions here are not arbitrary and are the difference
//! between a sweep and a random number generator:
//!
//! 1. **The baseline is measured first**, and every number in the run is
//!    stated against it. "25% smaller" is meaningless without saying than what.
//! 2. **The noise floor is measured before any timing comparison**, by running
//!    one unchanged binary repeatedly (`F0.5`).
//! 3. **Builds may run concurrently; benchmarks never do.** A benchmark timed
//!    while fifteen rustc processes are running measures the machine's load.
//!    When builds do run concurrently, build time is recorded as *unmeasured*
//!    rather than recorded wrong — and an unmeasured axis can never win on the
//!    frontier.

use binmap_core::configuration::BuildConfiguration;
use binmap_core::error::{Error, Result};
use binmap_core::event::{Cancellation, EngineEvent, EventSink, RunId};
use binmap_core::evidence::EvidenceId;
use binmap_core::finding::{Confidence, Finding, FindingDraft, FindingKind, Impact, Provenance};
use binmap_core::location::Location;
use binmap_core::traits::{BuildSystem, MeasurementSource, Target};
use binmap_measure::pareto::{self, Point};
use binmap_measure::timing::{NoiseFloor, Samples};
use binmap_measure::{measure_size, timing};
use binmap_verify::{
    BenchmarkVerdict, Candidate, Gate, GatePlan, Harness, SizeObservation, VerificationReport,
};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// How the sweep should run.
#[derive(Debug, Clone)]
pub struct SweepOptions {
    /// The cap on concurrent builds.
    pub parallelism: usize,
    /// How many times to run one unchanged binary to establish the machine's
    /// noise floor.
    pub noise_floor_samples: u32,
    /// The gates to apply to every configuration.
    pub gates: GatePlan,
}

impl SweepOptions {
    pub fn new(gates: GatePlan) -> Self {
        Self { parallelism: 1, noise_floor_samples: 7, gates }
    }

    pub fn with_parallelism(mut self, parallelism: usize) -> Self {
        self.parallelism = parallelism.max(1);
        self
    }

    /// Whether wall-clock build times mean anything in this run.
    ///
    /// They do not once builds overlap, and the honest response is to record
    /// no build time at all rather than one inflated by the sweep itself.
    pub fn build_time_is_measurable(&self) -> bool {
        self.parallelism == 1
    }
}

/// One configuration, measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasuredConfiguration {
    pub configuration: BuildConfiguration,
    /// The stable name, which is also the key a resumed sweep looks up.
    pub name: String,
    pub built: bool,
    /// Where the build put the artifact. Kept so a benchmark pass, which runs
    /// after every build, times the right binary — and re-checked before use,
    /// because a path that outlived its build is how a tool measures the wrong
    /// binary convincingly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<std::path::PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_time_nanos: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_nanos: Option<u64>,
    pub report: VerificationReport,
    pub evidence: Vec<EvidenceId>,
}

impl MeasuredConfiguration {
    fn point(&self) -> Point {
        let mut point = Point::new(self.name.clone(), self.size_bytes.unwrap_or(u64::MAX));
        if let Some(nanos) = self.build_time_nanos {
            point = point.with_build_time(nanos);
        }
        if let Some(nanos) = self.runtime_nanos {
            point = point.with_runtime(nanos);
        }
        if !self.report.passed() || !self.built {
            point = point.ineligible();
        }
        point
    }
}

/// A sweep in progress or interrupted, in a form that survives the session.
///
/// Cancellation keeps everything measured so far, and a resumed sweep skips
/// the configurations already in `measured` rather than repeating an
/// afternoon's builds (`F0.8`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepState {
    pub run: RunId,
    pub target: String,
    /// The full matrix this run was started against, so a resume knows what it
    /// still owes.
    pub planned: Vec<BuildConfiguration>,
    pub measured: Vec<MeasuredConfiguration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noise_floor: Option<NoiseFloor>,
    /// The baseline every number is stated against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_bytes: Option<u64>,
    pub complete: bool,
}

impl SweepState {
    pub fn new(run: RunId, target: impl Into<String>, planned: Vec<BuildConfiguration>) -> Self {
        Self {
            run,
            target: target.into(),
            planned,
            measured: Vec::new(),
            noise_floor: None,
            baseline_bytes: None,
            complete: false,
        }
    }

    /// The configurations still owed. A resume builds exactly these.
    pub fn remaining(&self) -> Vec<BuildConfiguration> {
        let done: std::collections::BTreeSet<String> =
            self.measured.iter().map(|m| m.name.clone()).collect();
        self.planned.iter().filter(|c| !done.contains(&c.name())).cloned().collect()
    }

    pub fn progress(&self) -> (usize, usize) {
        (self.measured.len(), self.planned.len())
    }

    /// The indices of the measured configurations on the Pareto frontier,
    /// smallest-first.
    pub fn frontier(&self) -> Vec<usize> {
        let points: Vec<Point> = self.measured.iter().map(MeasuredConfiguration::point).collect();
        pareto::frontier_by_size(&points)
    }

    /// The best size against the baseline, as a fraction. `0.25` is the
    /// twenty-five per cent Phase 0's acceptance criterion asks for.
    pub fn best_size_reduction(&self) -> Option<f64> {
        let baseline = self.baseline_bytes? as f64;
        if baseline == 0.0 {
            return None;
        }
        let best = self
            .measured
            .iter()
            .filter(|m| m.report.passed() && m.built)
            .filter_map(|m| m.size_bytes)
            .min()? as f64;
        Some((baseline - best) / baseline)
    }
}

/// Runs a matrix over one target.
pub struct Sweep<'a> {
    pub builder: &'a dyn BuildSystem,
    pub runner: &'a binmap_core::tool::ToolRunner,
    /// The user's benchmark, where they declared one. Runtime is measured
    /// through this and through nothing else.
    pub benchmark: Option<&'a dyn MeasurementSource>,
    pub options: SweepOptions,
}

impl<'a> Sweep<'a> {
    /// Run, or resume, a sweep.
    ///
    /// Returns whatever was measured even when cancelled: the state is the
    /// result, and a cancelled sweep is a shorter run rather than a lost one.
    pub fn run(
        &self,
        target: &Target,
        state: &mut SweepState,
        events: &dyn EventSink,
        cancellation: &Cancellation,
    ) -> Result<()> {
        let remaining = state.remaining();
        let (already, total) = state.progress();

        events.emit(EngineEvent::Started {
            run: state.run.clone(),
            description: if already == 0 {
                format!("Sweeping {total} configurations over {}", target.name)
            } else {
                format!("Resuming: {} of {total} configurations remain", remaining.len())
            },
            total: Some(total),
        });

        // 1. The baseline, and the machine, measured before anything is
        //    compared to anything.
        if state.baseline_bytes.is_none() {
            match self.measure_baseline(target, state, events, cancellation) {
                Ok(()) => {}
                Err(Error::Cancelled) => {
                    events.emit(EngineEvent::Cancelled {
                        run: state.run.clone(),
                        completed: state.measured.len(),
                    });
                    return Ok(());
                }
                Err(error) => {
                    events.emit(EngineEvent::Failed {
                        run: state.run.clone(),
                        error: error.to_string(),
                    });
                    return Err(error);
                }
            }
        }

        // 2. Build, size and gate every configuration, up to the cap.
        let measured = self.measure_all(target, &remaining, state, events, cancellation);
        state.measured.extend(measured);

        // 3. Benchmarks, strictly serially, over the survivors.
        if self.benchmark.is_some() {
            self.benchmark_survivors(state, events, cancellation);
        }

        if cancellation.is_cancelled() {
            events.emit(EngineEvent::Cancelled {
                run: state.run.clone(),
                completed: state.measured.len(),
            });
            return Ok(());
        }

        // 4. The frontier, derived from what was measured.
        state.complete = true;
        for index in state.frontier() {
            if let Ok(finding) = self.frontier_finding(state, index) {
                events.emit(EngineEvent::Finding {
                    run: state.run.clone(),
                    finding: Box::new(finding),
                });
            }
        }

        events.emit(EngineEvent::Finished {
            run: state.run.clone(),
            summary: self.summary(state),
        });
        Ok(())
    }

    fn summary(&self, state: &SweepState) -> String {
        let measured = state.measured.len();
        let passing = state.measured.iter().filter(|m| m.report.passed()).count();
        let frontier = state.frontier().len();
        match state.best_size_reduction() {
            Some(reduction) if reduction > 0.0 => format!(
                "{measured} configurations measured, {passing} passed every gate, \
                 {frontier} on the frontier; the smallest is {:.1}% under default release",
                reduction * 100.0
            ),
            _ => format!(
                "{measured} configurations measured, {passing} passed every gate, \
                 {frontier} on the frontier; none beat default release on size"
            ),
        }
    }

    /// Build default release, measure it, and time it repeatedly to learn how
    /// noisy this machine is.
    fn measure_baseline(
        &self,
        target: &Target,
        state: &mut SweepState,
        events: &dyn EventSink,
        cancellation: &Cancellation,
    ) -> Result<()> {
        cancellation.check()?;
        events.emit(EngineEvent::Progress {
            run: state.run.clone(),
            completed: 0,
            message: "Measuring the baseline: default release".into(),
        });

        let baseline = BuildConfiguration::default_release();
        let outcome = self.builder.build(target, &baseline)?;
        let artifact = outcome.artifact.ok_or_else(|| {
            Error::Other("default release does not build; there is nothing to compare to".into())
        })?;
        let (size, _) = measure_size(self.runner, &artifact)?;
        state.baseline_bytes = Some(size.total_bytes);

        // The noise floor, measured on this unchanged binary. Every timing
        // number in the run is shown beside it.
        if let Some(source) = self.benchmark {
            cancellation.check()?;
            events.emit(EngineEvent::Progress {
                run: state.run.clone(),
                completed: 0,
                message: format!(
                    "Establishing the noise floor over {} runs of the unchanged binary",
                    self.options.noise_floor_samples
                ),
            });
            let mut durations: Vec<Duration> = Vec::new();
            for _ in 0..self.options.noise_floor_samples {
                cancellation.check()?;
                durations.extend(source.samples(&artifact)?);
            }
            state.noise_floor = NoiseFloor::from_samples(Samples::new(durations));
        }
        Ok(())
    }

    /// Build, size and gate every configuration, up to `parallelism` at a time.
    fn measure_all(
        &self,
        target: &Target,
        configurations: &[BuildConfiguration],
        state: &SweepState,
        events: &dyn EventSink,
        cancellation: &Cancellation,
    ) -> Vec<MeasuredConfiguration> {
        let next = AtomicUsize::new(0);
        let done = AtomicUsize::new(state.measured.len());
        let results: Mutex<Vec<MeasuredConfiguration>> = Mutex::new(Vec::new());
        let workers = self.options.parallelism.min(configurations.len().max(1));

        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    loop {
                        if cancellation.is_cancelled() {
                            return;
                        }
                        let index = next.fetch_add(1, Ordering::SeqCst);
                        let Some(configuration) = configurations.get(index) else { return };

                        let measured = self.measure_one(target, configuration, state);
                        let completed = done.fetch_add(1, Ordering::SeqCst) + 1;

                        events.emit(EngineEvent::Progress {
                            run: state.run.clone(),
                            completed,
                            message: format!(
                                "{}: {}",
                                measured.name,
                                measured.report.summary().to_lowercase()
                            ),
                        });
                        if let Ok(finding) = self.configuration_finding(state, &measured) {
                            events.emit(EngineEvent::Finding {
                                run: state.run.clone(),
                                finding: Box::new(finding),
                            });
                        }
                        results.lock().expect("sweep results poisoned").push(measured);
                    }
                });
            }
        });

        let mut measured = results.into_inner().expect("sweep results poisoned");
        measured.sort_by(|a, b| a.name.cmp(&b.name));
        measured
    }

    fn measure_one(
        &self,
        target: &Target,
        configuration: &BuildConfiguration,
        state: &SweepState,
    ) -> MeasuredConfiguration {
        let name = configuration.name();
        let mut evidence = Vec::new();

        let outcome = self.builder.build(target, configuration);
        let (built, artifact, build_time) = match &outcome {
            Ok(outcome) => {
                evidence.push(outcome.evidence.clone());
                (outcome.succeeded, outcome.artifact.clone(), outcome.duration)
            }
            Err(_) => (false, None, Duration::ZERO),
        };

        let size_bytes = artifact.as_deref().and_then(|path| {
            measure_size(self.runner, path).ok().map(|(size, id)| {
                evidence.push(id);
                size.total_bytes
            })
        });

        let mut candidate = Candidate::new(name.clone()).citing(evidence.clone());
        if let (Some(baseline), Some(bytes)) = (state.baseline_bytes, size_bytes) {
            candidate =
                candidate.sized(SizeObservation { baseline_bytes: baseline, candidate_bytes: bytes });
        }

        // Gate the configuration. The build gate re-runs the build under the
        // same arguments, which cargo answers from its own cache — that is the
        // caching (`F0.8`) doing its job rather than a second full build.
        let report = Harness::new(self.runner, self.options.gates.clone()).verify(&candidate);
        for outcome in &report.outcomes {
            evidence.extend(outcome.evidence.iter().cloned());
        }

        MeasuredConfiguration {
            configuration: configuration.clone(),
            name,
            built,
            artifact,
            size_bytes,
            build_time_nanos: if self.options.build_time_is_measurable() && built {
                Some(build_time.as_nanos() as u64)
            } else {
                None
            },
            runtime_nanos: None,
            report,
            evidence,
        }
    }

    /// Time the configurations that passed their gates — one at a time, always.
    fn benchmark_survivors(
        &self,
        state: &mut SweepState,
        events: &dyn EventSink,
        cancellation: &Cancellation,
    ) {
        let Some(source) = self.benchmark else { return };
        let Some(floor) = state.noise_floor.clone() else { return };

        let baseline_samples = floor.samples.clone();
        for index in 0..state.measured.len() {
            if cancellation.is_cancelled() {
                return;
            }
            if !state.measured[index].report.passed() {
                continue;
            }
            let name = state.measured[index].name.clone();
            events.emit(EngineEvent::Progress {
                run: state.run.clone(),
                completed: index,
                message: format!("Timing {name}, one run at a time"),
            });

            // The artifact this configuration actually produced, confirmed to
            // still be there. A configuration whose binary has gone is skipped
            // rather than timed against whatever now sits at that path.
            let Some(artifact) =
                state.measured[index].artifact.clone().filter(|path| path.exists())
            else {
                continue;
            };
            let Ok(durations) = source.samples(&artifact) else { continue };
            let samples = Samples::new(durations);
            let verdict = timing::compare(&baseline_samples, &samples, &floor);
            state.measured[index].runtime_nanos =
                samples.median().map(|median| median.as_nanos() as u64);

            // A regression discovered here rejects a configuration that had
            // passed everything cheaper. It stays in the table, rejected by
            // the gate that caught it.
            if let BenchmarkVerdict::Regressed { detail } = &verdict
                && let Some(outcome) = state.measured[index]
                    .report
                    .outcomes
                    .iter_mut()
                    .find(|o| o.gate == Gate::BenchmarkNotWorse)
            {
                outcome.result = binmap_verify::GateResult::Failed;
                outcome.detail = detail.clone();
            }
        }
    }

    fn configuration_finding(
        &self,
        state: &SweepState,
        measured: &MeasuredConfiguration,
    ) -> Result<Finding> {
        let (kind, title) = if !measured.report.passed() {
            (
                FindingKind::RejectedCandidate,
                format!("{}: {}", measured.name, measured.report.summary()),
            )
        } else {
            (FindingKind::Configuration, format!("{}: measured", measured.name))
        };

        let impact = match (state.baseline_bytes, measured.size_bytes) {
            (Some(baseline), Some(bytes)) => Impact::size(bytes as i64 - baseline as i64),
            _ => Impact::none(),
        };

        Finding::new(
            FindingDraft::new(format!("cfg-{}", measured.name), kind, title)
                .detail(measured.configuration.describe())
                .at(Location::configuration(measured.name.clone()))
                .impact(impact)
                .confidence(Confidence::Certain)
                .provenance(Provenance::Measured)
                .citing(measured.evidence.clone()),
            self.runner.store(),
        )
    }

    fn frontier_finding(&self, state: &SweepState, index: usize) -> Result<Finding> {
        let measured = &state.measured[index];
        let impact = match (state.baseline_bytes, measured.size_bytes) {
            (Some(baseline), Some(bytes)) => Impact::size(bytes as i64 - baseline as i64),
            _ => Impact::none(),
        };
        let detail = format!(
            "Nothing else measured in this sweep beat it on every objective at once. {}",
            measured.configuration.describe()
        );

        Finding::new(
            FindingDraft::new(
                format!("frontier-{}", measured.name),
                FindingKind::FrontierPoint,
                format!("{} is on the frontier", measured.name),
            )
            .detail(detail)
            .at(Location::configuration(measured.name.clone()))
            .impact(impact)
            // Derived: a named rule of ours — Pareto dominance — concluded it
            // from measured inputs. The measurements are certain; this
            // conclusion about them is not the same thing.
            .confidence(Confidence::High)
            .provenance(Provenance::Derived { rule: "pareto-dominance".into() })
            .citing(measured.evidence.clone()),
            self.runner.store(),
        )
    }
}

#[cfg(test)]
mod tests;
