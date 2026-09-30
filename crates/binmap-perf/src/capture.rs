//! Getting a profile in the first place (`F3.1`, `TOOLING §4.3`).
//!
//! `§4.3` names two problems that stop a profiler working, and says what to do
//! about each: **detect the current state and print the exact command to
//! change it rather than failing opaquely.** Both are environment problems
//! rather than bugs, and both produce an empty profile with no explanation if
//! nobody checks.
//!
//! 1. **Permissions.** `kernel.perf_event_paranoid` must typically be ≤ 1, and
//!    ≤ -1 for full functionality. On this repository's own development
//!    machine it is 4, which fails every capture.
//! 2. **Frame pointers.** Rust historically omitted them, which makes `perf`'s
//!    default call-graph unwinding useless. The design is explicit that the
//!    behaviour must be *verified for the user's actual toolchain* rather than
//!    assumed, because it has changed and the answer decides the capture
//!    command.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// What stands between the user and a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Obstacle {
    /// `perf` is not installed.
    PerfMissing,
    /// `kernel.perf_event_paranoid` is too high.
    Paranoid { current: i32, needed: i32 },
    /// The binary has no frame pointers, so perf's cheap unwinding will
    /// produce one-frame stacks.
    NoFramePointers,
}

impl Obstacle {
    /// The exact command that fixes it.
    ///
    /// A command rather than a description, because "adjust your kernel
    /// settings" is advice nobody can act on at the moment they read it.
    pub fn remedy(&self) -> String {
        match self {
            Obstacle::PerfMissing => {
                "install it: `sudo apt install linux-tools-common linux-tools-generic`, or use \
                 samply, which needs no elevated permissions"
                    .into()
            }
            Obstacle::Paranoid { current, needed } => format!(
                "`sudo sysctl kernel.perf_event_paranoid={needed}` — it is {current}, and \
                 anything above {needed} refuses the capture. To make it survive a reboot, add \
                 `kernel.perf_event_paranoid={needed}` to /etc/sysctl.conf."
            ),
            Obstacle::NoFramePointers => {
                "rebuild with `RUSTFLAGS=-Cforce-frame-pointers=yes`, which costs a little \
                 runtime and gives much better captures — or capture with `--call-graph dwarf`, \
                 which needs no rebuild but produces far larger files"
                    .into()
            }
        }
    }

    /// What is lost if it is not fixed.
    ///
    /// Stated separately from the remedy because some obstacles are worth
    /// living with and the reader is the one who decides.
    pub fn costs(&self) -> &'static str {
        match self {
            Obstacle::PerfMissing => "There is no sampling source at all, so no profile.",
            Obstacle::Paranoid { .. } => {
                "Every capture fails. This is the one that has to be fixed."
            }
            Obstacle::NoFramePointers => {
                "Stacks will be one frame deep, so a flamegraph shows where the program was and \
                 nothing about how it got there. Self time is still correct."
            }
        }
    }

    /// Whether this stops a capture entirely.
    pub fn is_fatal(&self) -> bool {
        !matches!(self, Obstacle::NoFramePointers)
    }
}

/// What the machine can currently do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Readiness {
    pub obstacles: Vec<Obstacle>,
}

impl Readiness {
    pub fn can_capture(&self) -> bool {
        !self.obstacles.iter().any(Obstacle::is_fatal)
    }

    /// Everything wrong, with what to do about each.
    pub fn describe(&self) -> String {
        if self.obstacles.is_empty() {
            return "perf is available and permitted; captures will include call graphs.".into();
        }
        self.obstacles
            .iter()
            .map(|obstacle| format!("{} Fix: {}", obstacle.costs(), obstacle.remedy()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// The value the kernel requires for a useful capture.
///
/// 1 permits per-process sampling without raw tracepoints, which is everything
/// a profiler of one's own program needs.
pub const PARANOID_NEEDED: i32 = 1;

/// Check whether a profile can be captured.
///
/// `binary` is checked for frame pointers when supplied; without one only the
/// environment is examined.
pub fn readiness(binary: Option<&Path>) -> Readiness {
    let mut obstacles = Vec::new();

    if !on_path("perf") {
        obstacles.push(Obstacle::PerfMissing);
    }

    if let Some(current) = paranoid_level().filter(|level| *level > PARANOID_NEEDED) {
        obstacles.push(Obstacle::Paranoid { current, needed: PARANOID_NEEDED });
    }

    if let Some(binary) = binary {
        // Verified rather than assumed: `§4.3` says recent Rust changed the
        // default for `std` on some targets, and the answer decides the
        // capture command.
        if matches!(has_frame_pointers(binary), Some(false)) {
            obstacles.push(Obstacle::NoFramePointers);
        }
    }

    Readiness { obstacles }
}

/// The current `kernel.perf_event_paranoid`.
fn paranoid_level() -> Option<i32> {
    std::fs::read_to_string("/proc/sys/kernel/perf_event_paranoid").ok()?.trim().parse().ok()
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|directory| directory.join(program).is_file()))
        .unwrap_or(false)
}

/// Whether a binary keeps frame pointers.
///
/// Decided by looking at what function prologues actually do rather than by
/// asking the toolchain: `push rbp; mov rbp, rsp` is the frame-pointer
/// prologue on x86-64, and a binary either has them or does not whatever its
/// build claimed.
///
/// `None` when the binary cannot be read — an absent answer, not a negative
/// one, because reporting "no frame pointers" for a file we failed to open
/// would send someone to rebuild for no reason.
pub fn has_frame_pointers(binary: &Path) -> Option<bool> {
    use object::{Object, ObjectSection};

    let bytes = std::fs::read(binary).ok()?;
    let file = object::File::parse(&*bytes).ok()?;
    let text = file.section_by_name(".text")?;
    let code = text.uncompressed_data().ok()?;

    // `55` is `push rbp`; `48 89 e5` is `mov rbp, rsp`. Counting occurrences of
    // the pair against the size of .text is crude, and it does not need to be
    // better: the question is whether the build kept them at all, and the
    // answer is overwhelming in both directions.
    let prologues = code
        .windows(4)
        .filter(|window| window[0] == 0x55 && window[1..4] == [0x48, 0x89, 0xe5])
        .count();

    // One prologue per kilobyte of text is a build that keeps them; a build
    // that omits them still has a handful from assembly and from functions
    // that genuinely need a frame pointer.
    Some(prologues as u64 > code.len() as u64 / 1024)
}

/// The command to capture with, given what the machine can do.
///
/// Returned as arguments rather than a string, because a command a user is
/// told to run has to be one they can copy and one we can execute, and
/// building it twice is how those two diverge.
pub fn capture_command(binary: &Path, output: &Path, frame_pointers: bool) -> Vec<String> {
    let mut command = vec![
        "perf".into(),
        "record".into(),
        "-o".into(),
        output.display().to_string(),
        // 999 rather than 1000: a round frequency lands in lockstep with
        // periodic work and samples the same phase of it every time.
        "-F".into(),
        "999".into(),
        "-g".into(),
    ];
    if !frame_pointers {
        command.push("--call-graph".into());
        command.push("dwarf".into());
    }
    command.push("--".into());
    command.push(binary.display().to_string());
    command
}
