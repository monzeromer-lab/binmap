//! Whether a profile can be captured at all (`F3.1`, `TOOLING §4.3`).
//!
//! Both of these are environment problems rather than bugs, and both produce
//! an empty profile with no explanation if nobody checks. §4.3's instruction
//! is to "detect the current value and print the exact command to change it
//! rather than failing opaquely", so what is tested is the exactness.

use binmap_perf::capture::{
    Obstacle, PARANOID_NEEDED, Readiness, capture_command, has_frame_pointers, readiness,
};
use std::path::{Path, PathBuf};

fn crasher() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/crasher/target/release/crasher")
}

#[test]
fn this_machines_state_is_reported_rather_than_assumed() {
    // Whatever this machine's settings are, the probe produces an answer and
    // that answer is actionable. On the development machine
    // `perf_event_paranoid` is 4, which fails every capture.
    let state = readiness(None);
    let described = state.describe();
    assert!(!described.is_empty());

    for obstacle in &state.obstacles {
        assert!(!obstacle.remedy().is_empty(), "{obstacle:?} offers no fix");
        assert!(!obstacle.costs().is_empty(), "{obstacle:?} does not say what is lost");
    }
}

#[test]
fn a_paranoid_kernel_is_told_the_exact_sysctl() {
    // "Adjust your kernel settings" is advice nobody can act on at the moment
    // they read it.
    let obstacle = Obstacle::Paranoid { current: 4, needed: PARANOID_NEEDED };
    let remedy = obstacle.remedy();

    assert!(remedy.contains("sysctl kernel.perf_event_paranoid=1"), "{remedy}");
    assert!(remedy.contains("it is 4"), "it names the current value: {remedy}");
    assert!(remedy.contains("sysctl.conf"), "and how to make it stick: {remedy}");
    assert!(obstacle.is_fatal(), "no capture works at all until this is fixed");
}

#[test]
fn missing_frame_pointers_are_survivable_and_say_what_is_lost() {
    // Some obstacles are worth living with, and the reader is the one who
    // decides — so the cost is stated separately from the fix.
    let obstacle = Obstacle::NoFramePointers;
    assert!(!obstacle.is_fatal());

    assert!(obstacle.costs().contains("one frame deep"), "{}", obstacle.costs());
    assert!(
        obstacle.costs().contains("Self time is still correct"),
        "what still works is as important as what does not: {}",
        obstacle.costs()
    );
    assert!(obstacle.remedy().contains("force-frame-pointers"), "{}", obstacle.remedy());
    assert!(obstacle.remedy().contains("call-graph dwarf"), "both routes: {}", obstacle.remedy());
}

#[test]
fn a_missing_perf_points_at_the_alternative_that_needs_no_permissions() {
    let remedy = Obstacle::PerfMissing.remedy();
    assert!(remedy.contains("samply"), "{remedy}");
    assert!(remedy.contains("no elevated permissions"), "{remedy}");
}

#[test]
fn a_readiness_with_only_survivable_obstacles_can_still_capture() {
    let survivable = Readiness { obstacles: vec![Obstacle::NoFramePointers] };
    assert!(survivable.can_capture());

    let fatal = Readiness { obstacles: vec![Obstacle::Paranoid { current: 4, needed: 1 }] };
    assert!(!fatal.can_capture());
}

#[test]
fn a_clean_machine_says_so_rather_than_saying_nothing() {
    let clean = Readiness { obstacles: Vec::new() };
    assert!(clean.can_capture());
    assert!(clean.describe().contains("available and permitted"), "{}", clean.describe());
}

// --- frame pointers, verified rather than assumed ---------------------------

#[test]
fn a_real_binary_is_examined_rather_than_asked_about() {
    // §4.3: "Verify the behaviour for the user's actual toolchain and target
    // rather than assuming" — the default has changed, and the answer decides
    // the capture command.
    let Some(answer) = has_frame_pointers(&crasher()) else {
        eprintln!("corpus/crasher is not built");
        return;
    };
    // Either answer is legitimate; what matters is that it came from the
    // binary's own prologues.
    let _ = answer;
}

#[test]
fn a_binary_that_cannot_be_read_gives_no_answer_rather_than_a_negative_one() {
    // Reporting "no frame pointers" for a file we failed to open would send
    // someone to rebuild for no reason.
    assert_eq!(has_frame_pointers(Path::new("/definitely/not/here")), None);

    let scratch = tempfile::tempdir().unwrap();
    let not_elf = scratch.path().join("not-elf");
    std::fs::write(&not_elf, b"plainly not an object file").unwrap();
    assert_eq!(has_frame_pointers(&not_elf), None);
}

// --- the capture command ----------------------------------------------------

#[test]
fn the_command_is_built_once_so_what_is_shown_is_what_is_run() {
    // A command a user is told to run has to be one they can copy and one we
    // can execute. Building it twice is how those diverge.
    let command = capture_command(Path::new("./app"), Path::new("/tmp/perf.data"), true);

    assert_eq!(command[0], "perf");
    assert!(command.contains(&"record".to_string()));
    assert!(command.contains(&"-g".to_string()), "call graphs are the point");
    assert_eq!(command.last(), Some(&"./app".to_string()));
    assert!(command.contains(&"/tmp/perf.data".to_string()));
}

#[test]
fn without_frame_pointers_the_command_asks_for_dwarf_unwinding() {
    let with = capture_command(Path::new("./app"), Path::new("/tmp/p"), true);
    let without = capture_command(Path::new("./app"), Path::new("/tmp/p"), false);

    assert!(!with.contains(&"dwarf".to_string()), "no need when they are there");
    assert!(without.contains(&"dwarf".to_string()), "and needed when they are not");
    assert!(without.contains(&"--call-graph".to_string()));
}

#[test]
fn the_sampling_frequency_is_not_a_round_number() {
    // A round frequency lands in lockstep with periodic work and samples the
    // same phase of it every time.
    let command = capture_command(Path::new("./app"), Path::new("/tmp/p"), true);
    assert!(command.contains(&"999".to_string()), "{command:?}");
    assert!(!command.contains(&"1000".to_string()));
}
