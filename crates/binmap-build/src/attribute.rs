//! Running size attribution as an analysis (`F1.2`–`F1.4`).
//!
//! The reading and grouping live in `binmap-binary`; this is what turns them
//! into findings, which is the only form the rest of the product understands.
//!
//! Every finding here is `Measured` or `Derived`, never inferred. Reading a
//! symbol table is a measurement; concluding that twelve instantiations of one
//! generic could be collapsed is a named rule applied to measurements. Neither
//! needs a model, and `§2.4` says an analysis must not call one.

use binmap_binary::symbols::SymbolTable;
use binmap_core::attribution::{Attribution, Driver};
use binmap_core::configuration::BuildConfiguration;
use binmap_core::error::Result;
use binmap_core::event::{Cancellation, EngineEvent, EventSink, RunId};
use binmap_core::evidence::{EvidenceId, ToolInvocation};
use binmap_core::finding::{Confidence, Finding, FindingDraft, FindingKind, Impact, Provenance};
use binmap_core::location::Location;
use binmap_core::tool::ToolRunner;
use binmap_core::traits::{BuildSystem, Target};
use std::path::Path;

/// One attribution run, and everything it found.
pub struct SizeAnalysis<'a> {
    pub builder: &'a dyn BuildSystem,
    pub runner: &'a ToolRunner,
    /// The crates the user wrote, so "your code" can be told from "a
    /// dependency". There is no marker in a symbol name for it.
    pub own_crates: Vec<String>,
}

impl SizeAnalysis<'_> {
    /// Build the target and attribute what comes out.
    ///
    /// Does not emit a terminal event: the caller does, once it has persisted
    /// whatever this found. See the note at the end of the method.
    pub fn run(
        &self,
        run: RunId,
        target: &Target,
        events: &dyn EventSink,
        cancellation: &Cancellation,
    ) -> Result<Attribution> {
        events.emit(EngineEvent::Started {
            run: run.clone(),
            description: format!("Attributing {}", target.name),
            total: Some(3),
        });

        cancellation.check()?;
        events.emit(EngineEvent::Progress {
            run: run.clone(),
            completed: 1,
            message: "Building the target as it ships".into(),
        });
        let outcome = self.builder.build(target, &BuildConfiguration::default_release())?;
        let artifact = outcome.artifact.ok_or_else(|| {
            binmap_core::Error::Other(format!("{} did not produce an artifact", target.name))
        })?;

        cancellation.check()?;
        events.emit(EngineEvent::Progress {
            run: run.clone(),
            completed: 2,
            message: "Reading the symbol table".into(),
        });
        let (table, evidence) = self.read(&artifact)?;

        if table.stripped {
            // Not a failure. A stripped binary is a normal thing to ship, and
            // the honest response is to say what was lost rather than report
            // an empty attribution as though it were the answer.
            events.emit(EngineEvent::Progress {
                run,
                completed: 3,
                message: format!(
                    "{} is stripped, so only its exported symbols are visible. Build with \
                     strip = \"none\" or strip = \"debuginfo\" to attribute all of it.",
                    artifact.display()
                ),
            });
            return Ok(binmap_binary::attribute(&table, &self.own_crates));
        }

        cancellation.check()?;
        events.emit(EngineEvent::Progress {
            run: run.clone(),
            completed: 3,
            message: format!("Attributing {} symbols", table.symbols.len()),
        });
        let attribution = binmap_binary::attribute(&table, &self.own_crates);

        for finding in self.findings(&attribution, &evidence, &artifact) {
            events.emit(EngineEvent::Finding { run: run.clone(), finding: Box::new(finding) });
        }

        // The terminal event is deliberately *not* emitted here.
        //
        // A terminal event means the run is over, and anything waiting on one
        // acts the moment it arrives — the headless harness exits on it.
        // Emitting it before the session had been written meant the process
        // died mid-write and the attribution was simply lost, silently, with
        // the run reporting success. The caller emits it after persisting,
        // which is the ordering the sweep already keeps.
        let _ = run;
        Ok(attribution)
    }

    /// Read the table, recording the read as evidence.
    fn read(&self, artifact: &Path) -> Result<(SymbolTable, EvidenceId)> {
        let pending = self
            .runner
            .store()
            .begin(ToolInvocation::new("binmap:read-symbols", [artifact.display().to_string()]));

        match SymbolTable::read(artifact) {
            Ok(table) => {
                // The record is a summary rather than every symbol: a hundred
                // thousand lines of verbatim output is not evidence anyone
                // reads, and the digest would be over noise.
                let report = format!(
                    "{}\n{} symbols, {} bytes, {:.1}% of sizes inferred, mangling {:?}\n",
                    artifact.display(),
                    table.symbols.len(),
                    table.total_bytes(),
                    table.inferred_fraction * 100.0,
                    table.mangling()
                );
                let evidence = self.runner.store().complete(pending, report, 0);
                Ok((table, evidence))
            }
            Err(error) => {
                self.runner.store().complete(pending, error.to_string(), -1);
                Err(error)
            }
        }
    }

    /// The one-line result, for the caller's terminal event.
    pub fn summary(&self, attribution: &Attribution) -> String {
        let yours = attribution
            .drivers
            .iter()
            .find(|(driver, _)| *driver == Driver::Yours)
            .map(|(_, bytes)| *bytes)
            .unwrap_or(0);
        format!(
            "{} bytes attributed across {} crates; {} is your code, and {} generics have more \
             than one instantiation",
            attribution.attributed_bytes,
            attribution.crates.len(),
            human(yours),
            attribution.monomorphizations.len()
        )
    }

    /// Turn an attribution into findings.
    ///
    /// Two kinds, and the difference is the difference between measuring and
    /// concluding: a size driver is what the symbol table says, so it is
    /// Measured and Certain. A collapsible monomorphization is a rule of ours
    /// applied to that — so it is Derived, capped at High, and says which rule.
    fn findings(
        &self,
        attribution: &Attribution,
        evidence: &EvidenceId,
        artifact: &Path,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();
        let total = attribution.attributed_bytes.max(1);

        // The recurring drivers (F1.4), largest first, and only the ones worth
        // a line. A category under one per cent is noise in a list.
        for (driver, bytes) in attribution.drivers.iter().take(8) {
            let share = *bytes as f64 * 100.0 / total as f64;
            if share < 1.0 {
                continue;
            }
            let mut detail =
                format!("{bytes} bytes, {share:.1}% of what the symbol table accounts for.");
            if let Some(remedy) = driver.remedy() {
                detail.push(' ');
                detail.push_str(remedy);
            }

            let draft = FindingDraft::new(
                format!("driver-{driver:?}").to_lowercase(),
                FindingKind::SizeDriver,
                format!("{} accounts for {}", driver.label(), human(*bytes)),
            )
            .detail(detail)
            .at(Location::symbol(driver.label()).with_unit(artifact.display().to_string()))
            .impact(Impact::size(*bytes as i64))
            .confidence(Confidence::Certain)
            .provenance(Provenance::Measured)
            .cite(evidence.clone());

            if let Ok(finding) = Finding::new(draft, self.runner.store()) {
                findings.push(finding);
            }
        }

        // The generics worth collapsing (F1.3). Ranked by what collapsing
        // would save, not by instantiation count: twelve copies of a tiny
        // function matter less than three of a large one.
        for monomorphization in attribution.monomorphizations.iter().take(10) {
            let saving = monomorphization.collapsible_bytes();
            if saving < 1024 {
                continue;
            }
            let arguments: Vec<String> = monomorphization
                .arguments
                .iter()
                .take(4)
                .map(|(argument, bytes)| format!("{argument} ({})", human(*bytes)))
                .collect();

            let draft = FindingDraft::new(
                format!("mono-{}", monomorphization.generic_path),
                FindingKind::Monomorphization,
                format!(
                    "{} is instantiated {} times, costing {}",
                    monomorphization.generic_path,
                    monomorphization.instantiations,
                    human(monomorphization.total_bytes)
                ),
            )
            .detail(format!(
                "Collapsing every instantiation to one would save at most {} — the largest has \
                 to stay. An upper bound: collapsing usually costs an indirection, and two \
                 instantiations that differ only in a type parameter may still differ in what \
                 the optimiser did to them. Arguments seen: {}.",
                human(saving),
                arguments.join(", ")
            ))
            .at(Location::symbol(monomorphization.generic_path.clone()))
            .impact(Impact::size(-(saving as i64)))
            // Derived, not measured: the sizes are certain, the conclusion
            // that they can be collapsed is a rule of ours.
            .confidence(Confidence::High)
            .provenance(Provenance::Derived { rule: "generic-instantiation-grouping".into() })
            .cite(evidence.clone());

            if let Ok(finding) = Finding::new(draft, self.runner.store()) {
                findings.push(finding);
            }
        }

        findings
    }
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut amount = bytes as f64;
    let mut unit = 0;
    while amount >= 1024.0 && unit + 1 < UNITS.len() {
        amount /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{amount:.1} {}", UNITS[unit]) }
}
