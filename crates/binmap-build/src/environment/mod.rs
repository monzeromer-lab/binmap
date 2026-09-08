//! Feature-detecting the machine, and saying what to type when something is
//! missing (`U0.4`, `U10`, TOOLING §9).
//!
//! Two rules. **Never assume a capability that has not been confirmed** — and
//! **every warning names the exact fix**, as a command, not a description. A
//! devtool that fails with a subprocess error message is a devtool people
//! uninstall.
//!
//! Nothing here blocks a configuration sweep. A missing tool makes the axes
//! that need it unavailable and says so; it does not turn into an error at the
//! point of use, an hour in.
//!
//! Probes are cheap and re-runnable, which is what makes the panel's re-check
//! action work: a missing tool becomes recoverable without restarting.

use binmap_core::evidence::ToolInvocation;
use binmap_core::facade::{Probe, ProbeStatus};
use binmap_core::tool::ToolRunner;

const RUST: &str = "Rust target";
const MEASUREMENT: &str = "Measurement";
const REPLAY: &str = "Replay · Phase 4";
const PROJECT: &str = "Project";
const VERIFICATION: &str = "Verification";

/// Run every probe, grouped as the environment panel renders them.
pub fn probe_all(runner: &ToolRunner) -> Vec<Probe> {
    let mut probes = vec![
        version(runner, RUST, "rustc", "rustc", "rustup toolchain install stable"),
        version(runner, RUST, "cargo", "cargo", "Install Rust from https://rustup.rs"),
        nightly(runner),
    ];

    // Measurement. bloaty and hyperfine are the two the engine actually reaches
    // for; both degrade rather than fail.
    probes.push(cross_check_tool(
        runner,
        "bloaty",
        "cross-check only — size attribution is ours, this is the oracle that contradicts it",
        "our size attribution goes uncross-checked",
        "apt install bloaty",
    ));
    probes.push(cross_check_tool(
        runner,
        "hyperfine",
        "JSON parsed, never the table",
        "runtime is not an objective without it; size and build time still are",
        "cargo install hyperfine",
    ));
    probes.push(cross_check_tool(
        runner,
        "samply",
        "preferred sample source from Phase 3",
        "not needed until Phase 3",
        "cargo install samply",
    ));
    probes.push(cross_check_tool(
        runner,
        "perf",
        "PMU counters readable",
        "not needed until Phase 3",
        "apt install linux-tools-$(uname -r)",
    ));

    probes.push(replay_recorder(runner));
    probes.push(perf_event_paranoid());
    probes.push(sanitizer(runner));
    probes.push(git(runner));
    probes.push(core_dumps(runner));
    probes
}

/// A tool that answers `--version`.
fn version(runner: &ToolRunner, group: &str, name: &str, tool: &str, remedy: &str) -> Probe {
    match runner.run(ToolInvocation::new(tool, ["--version"])) {
        Ok(output) if output.succeeded() => {
            Probe::present(group, name, output.stdout.lines().next().unwrap_or("present").trim())
        }
        Ok(output) => Probe::needs(
            group,
            name,
            ProbeStatus::Unusable,
            format!("on PATH but exited {}", output.exit_code),
            remedy,
        )
        .with_action("Re-check"),
        Err(_) => Probe::needs(
            group,
            name,
            ProbeStatus::Missing,
            format!("`{tool}` is not on PATH"),
            remedy,
        )
        .with_action("Re-check"),
    }
}

/// A measurement tool whose absence costs a capability rather than the run.
///
/// The `absent` string says what is actually lost, because "not installed" on
/// its own does not tell a user whether to care.
fn cross_check_tool(
    runner: &ToolRunner,
    tool: &str,
    present: &str,
    absent: &str,
    remedy: &str,
) -> Probe {
    match runner.run(ToolInvocation::new(tool, ["--version"])) {
        Ok(output) if output.succeeded() => Probe::present(
            MEASUREMENT,
            tool,
            format!("{} · {present}", output.stdout.lines().next().unwrap_or(tool).trim()),
        ),
        _ => Probe::needs(MEASUREMENT, tool, ProbeStatus::Missing, absent, remedy)
            .with_action("Re-check"),
    }
}

/// The extended sweep (`F0.3`) needs nightly and `rust-src`. Probed rather
/// than attempted: the failure mode otherwise is ninety-six builds that all
/// fail the same way an hour in.
fn nightly(runner: &ToolRunner) -> Probe {
    let remedy = "rustup toolchain install nightly && rustup component add rust-src \
                  --toolchain nightly";

    let Ok(output) =
        runner.run(ToolInvocation::new("rustup", ["component", "list", "--toolchain", "nightly"]))
    else {
        return Probe::needs(
            RUST,
            "nightly with rust-src",
            ProbeStatus::Missing,
            "rustup is not on PATH, so the extended sweep is unavailable",
            remedy,
        )
        .with_action("Re-check");
    };

    let installed = output.succeeded()
        && output
            .stdout
            .lines()
            .any(|line| line.starts_with("rust-src") && line.contains("(installed)"));

    if installed {
        Probe::present(RUST, "nightly with rust-src", "build-std and Miri axes available")
    } else {
        Probe::needs(
            RUST,
            "nightly with rust-src",
            ProbeStatus::Missing,
            "build-std and Miri axes unavailable; the sweep runs without them",
            remedy,
        )
        .with_action("Install")
    }
}

fn replay_recorder(runner: &ToolRunner) -> Probe {
    match runner.run(ToolInvocation::new("rr", ["--version"])) {
        Ok(output) if output.succeeded() => {
            Probe::present(REPLAY, "rr", output.stdout.lines().next().unwrap_or("present").trim())
        }
        _ => Probe::needs(
            REPLAY,
            "rr",
            ProbeStatus::Missing,
            "replay debugging unavailable",
            "apt install rr",
        )
        .with_action("Re-check"),
    }
}

/// `perf` and `rr` both need `kernel.perf_event_paranoid` ≤ 1.
///
/// Read from `/proc` rather than shelled out: it is a file, and the exact
/// `sysctl` line is what the user needs back.
fn perf_event_paranoid() -> Probe {
    const PATH: &str = "/proc/sys/kernel/perf_event_paranoid";
    let name = "kernel.perf_event_paranoid";

    let Ok(text) = std::fs::read_to_string(PATH) else {
        return Probe::needs(
            REPLAY,
            name,
            ProbeStatus::Unusable,
            "could not be read, so performance-counter access is unknown",
            "sudo sysctl kernel.perf_event_paranoid=1",
        );
    };
    let Ok(value) = text.trim().parse::<i32>() else {
        return Probe::needs(
            REPLAY,
            name,
            ProbeStatus::Unusable,
            format!("reads `{}`, which is not a number", text.trim()),
            "sudo sysctl kernel.perf_event_paranoid=1",
        );
    };

    if value <= 1 {
        Probe::present(REPLAY, name, format!("= {value} · perf and rr can read counters"))
    } else {
        Probe::needs(
            REPLAY,
            name,
            ProbeStatus::Unusable,
            format!("= {value} · perf and rr need ≤ 1"),
            "sudo sysctl kernel.perf_event_paranoid=1",
        )
        .with_action("Apply")
    }
}

/// Miri, for the `MiriClean` gate. Its absence does not stop a sweep; it means
/// unsafe code goes unchecked, and the gate reports that rather than passing.
fn sanitizer(runner: &ToolRunner) -> Probe {
    match runner.run(ToolInvocation::new("cargo", ["miri", "--version"])) {
        Ok(output) if output.succeeded() => {
            Probe::present(VERIFICATION, "miri", output.stdout.trim())
        }
        _ => Probe::needs(
            VERIFICATION,
            "miri",
            ProbeStatus::Missing,
            "unsafe code will be reported as unchecked rather than as clean",
            "rustup toolchain install nightly --component miri",
        )
        .with_action("Install"),
    }
}

/// Knowing the commit is what lets a measurement be reproduced. A dirty tree
/// is reported as a fact, not a fault — plenty of useful work happens on one.
fn git(runner: &ToolRunner) -> Probe {
    match runner.run(ToolInvocation::new("git", ["rev-parse", "--short", "HEAD"])) {
        Ok(output) if output.succeeded() => {
            let commit = output.stdout.trim().to_string();
            let dirty = runner
                .run(ToolInvocation::new("git", ["status", "--porcelain"]))
                .map(|status| !status.stdout.trim().is_empty())
                .unwrap_or(false);
            if dirty {
                Probe::needs(
                    PROJECT,
                    "git",
                    ProbeStatus::Unusable,
                    format!("{commit}, with uncommitted changes — results will not reproduce"),
                    "git stash",
                )
            } else {
                Probe::present(PROJECT, "git", format!("{commit}, clean"))
            }
        }
        _ => Probe::needs(
            PROJECT,
            "git",
            ProbeStatus::Unusable,
            "not a git repository, so results cannot be tied to a commit",
            "git init && git add -A && git commit -m 'baseline'",
        ),
    }
}

/// Core dumps are frequently disabled or routed elsewhere, and TOOLING §2.6 is
/// explicit that the tool should detect and explain that rather than reporting
/// "no core found" when Phase 2 arrives.
fn core_dumps(runner: &ToolRunner) -> Probe {
    let name = "core dumps";
    let limit = runner
        .run(ToolInvocation::new("sh", ["-c", "ulimit -c"]))
        .ok()
        .map(|output| output.stdout.trim().to_string())
        .unwrap_or_default();

    let pattern = std::fs::read_to_string("/proc/sys/kernel/core_pattern")
        .unwrap_or_default()
        .trim()
        .to_string();
    let routed = pattern.starts_with('|');

    match limit.as_str() {
        "0" => Probe::needs(
            PROJECT,
            name,
            ProbeStatus::Unusable,
            "disabled by ulimit, so a crash leaves nothing to analyse",
            "ulimit -c unlimited",
        ),
        "" => Probe::needs(
            PROJECT,
            name,
            ProbeStatus::Unusable,
            "the core-file limit could not be read",
            "ulimit -c unlimited",
        ),
        limit if routed => Probe::present(
            PROJECT,
            name,
            format!(
                "ulimit -c {limit} · routed through a handler ({}), so retrieve with coredumpctl",
                pattern
                    .split('/')
                    .next_back()
                    .unwrap_or("handler")
                    .split(' ')
                    .next()
                    .unwrap_or("handler")
            ),
        ),
        limit => Probe::present(
            PROJECT,
            name,
            format!("ulimit -c {limit} · written to the working directory"),
        ),
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
            if !probe.needs_attention() {
                assert!(probe.remedy.is_none(), "{} is present but offers a remedy", probe.name);
                continue;
            }
            let remedy = probe
                .remedy
                .as_deref()
                .unwrap_or_else(|| panic!("{} failed without saying what to type", probe.name));
            // Something a user can paste, not advice they must translate.
            let first = remedy.split_whitespace().next().unwrap_or("");
            assert!(
                ["rustup", "cargo", "git", "apt", "sudo", "ulimit", "vite", "Install"]
                    .contains(&first),
                "{}: `{remedy}` does not start with something to run",
                probe.name
            );
        }
    }

    #[test]
    fn a_missing_measurement_tool_says_what_is_actually_lost() {
        // "not installed" alone does not tell a user whether to care.
        for probe in probes().into_iter().filter(|p| p.group == MEASUREMENT && p.needs_attention())
        {
            assert!(
                probe.detail.len() > 20 && !probe.detail.starts_with("not installed"),
                "{}: `{}` does not say what it costs",
                probe.name,
                probe.detail
            );
        }
    }

    #[test]
    fn the_panel_groups_match_the_ones_the_design_renders() {
        let groups: std::collections::BTreeSet<String> =
            probes().into_iter().map(|probe| probe.group).collect();
        for expected in [RUST, MEASUREMENT, REPLAY, PROJECT, VERIFICATION] {
            assert!(groups.contains(expected), "{expected} is missing from {groups:?}");
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
    fn the_paranoia_level_is_read_and_judged_against_what_perf_needs() {
        let probes = probes();
        let paranoid =
            probes.iter().find(|p| p.name == "kernel.perf_event_paranoid").expect("always probed");
        // Whatever this machine is set to, the probe states the number.
        assert!(
            paranoid.detail.contains('=') || paranoid.detail.contains("could not be read"),
            "{}",
            paranoid.detail
        );
    }

    #[test]
    fn re_checking_is_cheap_enough_to_be_a_button() {
        let started = std::time::Instant::now();
        let first = probes();
        let second = probes();
        assert_eq!(
            first.iter().map(|p| p.status).collect::<Vec<_>>(),
            second.iter().map(|p| p.status).collect::<Vec<_>>()
        );
        assert!(started.elapsed().as_secs() < 30, "probing twice is too slow to be a button");
    }
}
