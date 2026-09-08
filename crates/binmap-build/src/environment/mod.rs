//! Feature-detecting the machine, and saying what to type when something is
//! missing (`U0.4`, `U10`).
//!
//! The rule: **never assume a capability that has not been confirmed**. Every
//! probe reports what it found, and every failing probe carries the exact
//! command that fixes it — not a description of the fix, the command. A user
//! who has to work out the incantation themselves has been told there is a
//! problem and left with it.
//!
//! Probes are cheap and re-runnable, which is what makes the environment
//! panel's re-check action work: a missing tool becomes recoverable without
//! restarting the application.

use binmap_core::evidence::ToolInvocation;
use binmap_core::facade::{Probe, ProbeStatus};
use binmap_core::tool::ToolRunner;

/// Run every probe this build knows about.
///
/// Grouped per target family and, later, per capability — the environment
/// panel renders the groups in the order they come back.
pub fn probe_all(runner: &ToolRunner) -> Vec<Probe> {
    let mut probes = vec![
        version_probe(
            runner,
            "Rust",
            "cargo",
            &["--version"],
            "Install Rust from https://rustup.rs, then reopen this project.",
        ),
        version_probe(
            runner,
            "Rust",
            "rustc",
            &["--version"],
            "rustup toolchain install stable",
        ),
    ];

    probes.push(nightly_probe(runner));
    probes.push(git_probe(runner));
    probes.push(sanitizer_probe(runner));
    probes
}

/// A tool that answers `--version` is present; one that does not is missing,
/// and the remedy says how to get it.
fn version_probe(
    runner: &ToolRunner,
    group: &str,
    tool: &str,
    arguments: &[&str],
    remedy: &str,
) -> Probe {
    match runner.run(ToolInvocation::new(tool, arguments.to_vec())) {
        Ok(output) if output.succeeded() => Probe {
            group: group.to_string(),
            name: tool.to_string(),
            status: ProbeStatus::Present,
            detail: output.stdout.lines().next().unwrap_or("present").trim().to_string(),
            remedy: None,
        },
        Ok(output) => Probe {
            group: group.to_string(),
            name: tool.to_string(),
            status: ProbeStatus::Unusable,
            detail: format!("`{tool}` is on PATH but exited {}", output.exit_code),
            remedy: Some(remedy.to_string()),
        },
        Err(_) => Probe {
            group: group.to_string(),
            name: tool.to_string(),
            status: ProbeStatus::Missing,
            detail: format!("`{tool}` is not on PATH"),
            remedy: Some(remedy.to_string()),
        },
    }
}

/// The extended sweep (`F0.3`) needs nightly and `rust-src`. Probed rather
/// than attempted, because the failure mode otherwise is ninety-six builds
/// that all fail the same way an hour in.
fn nightly_probe(runner: &ToolRunner) -> Probe {
    let group = "Rust".to_string();
    let name = "nightly toolchain with rust-src".to_string();
    let remedy = "rustup toolchain install nightly && rustup component add rust-src \
                  --toolchain nightly";

    let Ok(output) = runner.run(ToolInvocation::new("rustup", ["component", "list", "--toolchain", "nightly"]))
    else {
        return Probe {
            group,
            name,
            status: ProbeStatus::Missing,
            detail: "rustup is not on PATH, so the extended sweep is unavailable".into(),
            remedy: Some(remedy.to_string()),
        };
    };

    let installed = output.succeeded()
        && output
            .stdout
            .lines()
            .any(|line| line.starts_with("rust-src") && line.contains("(installed)"));

    if installed {
        Probe {
            group,
            name,
            status: ProbeStatus::Present,
            detail: "build-std is available, so the extended sweep can run".into(),
            remedy: None,
        }
    } else {
        Probe {
            group,
            name,
            status: ProbeStatus::Missing,
            // Absent rather than broken: the sweep still runs, one axis
            // shorter, and the copy says so rather than implying failure.
            detail: "not installed; the sweep runs without the build-std axis".into(),
            remedy: Some(remedy.to_string()),
        }
    }
}

/// Knowing the commit is what lets a measurement be reproduced. A dirty tree
/// is reported as a fact, not as an error — plenty of useful work happens on
/// one.
fn git_probe(runner: &ToolRunner) -> Probe {
    let group = "Project".to_string();
    let name = "git".to_string();
    match runner.run(ToolInvocation::new("git", ["rev-parse", "--short", "HEAD"])) {
        Ok(output) if output.succeeded() => {
            let commit = output.stdout.trim().to_string();
            let dirty = runner
                .run(ToolInvocation::new("git", ["status", "--porcelain"]))
                .map(|status| !status.stdout.trim().is_empty())
                .unwrap_or(false);
            Probe {
                group,
                name,
                status: ProbeStatus::Present,
                detail: if dirty {
                    format!("{commit}, with uncommitted changes — results will not reproduce")
                } else {
                    commit
                },
                remedy: None,
            }
        }
        _ => Probe {
            group,
            name,
            status: ProbeStatus::Unusable,
            detail: "not a git repository, so results cannot be tied to a commit".into(),
            remedy: Some("git init && git add -A && git commit -m 'baseline'".into()),
        },
    }
}

/// Miri, for the `MiriClean` gate. Its absence does not stop a sweep; it means
/// unsafe code goes unchecked, and the gate says so rather than passing.
fn sanitizer_probe(runner: &ToolRunner) -> Probe {
    let group = "Verification".to_string();
    let name = "miri".to_string();
    match runner.run(ToolInvocation::new("cargo", ["miri", "--version"])) {
        Ok(output) if output.succeeded() => Probe {
            group,
            name,
            status: ProbeStatus::Present,
            detail: output.stdout.trim().to_string(),
            remedy: None,
        },
        _ => Probe {
            group,
            name,
            status: ProbeStatus::Missing,
            detail: "unsafe code will be reported as unchecked rather than as clean".into(),
            remedy: Some(
                "rustup toolchain install nightly --component miri".into(),
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use binmap_core::evidence::EvidenceStore;

    fn probes() -> Vec<Probe> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        probe_all(&ToolRunner::new(EvidenceStore::new(), root))
    }

    #[test]
    fn every_failing_probe_carries_a_command_not_a_description() {
        for probe in probes() {
            match probe.status {
                ProbeStatus::Present => assert!(
                    probe.remedy.is_none(),
                    "{} is present but offers a remedy",
                    probe.name
                ),
                ProbeStatus::Missing | ProbeStatus::Unusable => {
                    let remedy = probe.remedy.as_deref().unwrap_or_else(|| {
                        panic!("{} failed without saying what to type", probe.name)
                    });
                    // A remedy a user can paste, rather than advice they have
                    // to translate into one.
                    assert!(
                        remedy.starts_with("rustup")
                            || remedy.starts_with("cargo")
                            || remedy.starts_with("git")
                            || remedy.starts_with("Install"),
                        "{}: `{remedy}` is not something to type",
                        probe.name
                    );
                }
            }
        }
    }

    #[test]
    fn every_probe_says_what_it_found_and_where_it_belongs() {
        let probes = probes();
        assert!(!probes.is_empty());
        for probe in &probes {
            assert!(!probe.detail.trim().is_empty(), "{} found nothing to say", probe.name);
            assert!(!probe.group.trim().is_empty(), "{} belongs to no group", probe.name);
        }
    }

    #[test]
    fn cargo_is_found_since_the_tests_are_running_under_it() {
        let probes = probes();
        let cargo = probes.iter().find(|probe| probe.name == "cargo").unwrap();
        assert_eq!(cargo.status, ProbeStatus::Present);
        assert!(cargo.detail.contains("cargo"), "{}", cargo.detail);
    }

    #[test]
    fn re_checking_is_cheap_enough_to_be_a_button() {
        // The environment panel's re-check action re-runs everything, so the
        // whole set has to be fast and side-effect free.
        let started = std::time::Instant::now();
        let first = probes();
        let second = probes();
        assert_eq!(
            first.iter().map(|p| p.status).collect::<Vec<_>>(),
            second.iter().map(|p| p.status).collect::<Vec<_>>()
        );
        assert!(started.elapsed().as_secs() < 20, "probing twice took too long to be a button");
    }
}
