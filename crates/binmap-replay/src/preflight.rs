//! Whether a recording can be made at all (`F4.1`, `TOOLING §5`).
//!
//! `F4.1` asks for "the recording constraints stated before a user's first
//! failed attempt rather than after", and `§5` lists them: `rr` needs specific
//! CPU features and performance counter access, is Linux x86-64 only, needs
//! `kernel.perf_event_paranoid ≤ 1`, and "has known incompatibilities in some
//! virtualized environments".
//!
//! None of that is disqualifying and all of it is invisible until a recording
//! fails, usually after someone has waited for a slow one. So every constraint
//! is checked up front, each answer says what it costs and the exact command
//! that fixes it, and a machine that cannot record is told so before it tries.
//!
//! This machine fails three of them, which is why the checks are written
//! against real conditions rather than imagined ones.

use serde::{Deserialize, Serialize};

/// Something standing between the user and a recording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Obstacle {
    /// `rr` is not installed.
    NotInstalled,
    /// Not Linux, or not x86-64.
    WrongPlatform { found: String },
    /// `kernel.perf_event_paranoid` is too high.
    ///
    /// The same constraint the sampling profiler hits, for the same reason:
    /// `rr` counts retired conditional branches to know where it is in a
    /// replay, and that is a performance counter.
    Paranoid { current: i32, needed: i32 },
    /// The CPU's performance counters are not usable.
    NoPerformanceCounters { because: String },
    /// Running under a hypervisor that `rr` has trouble with.
    ///
    /// A warning rather than a refusal: some virtualized environments work and
    /// some do not, and refusing outright would stop people whose setup is
    /// fine.
    Virtualized { hypervisor: String },
}

impl Obstacle {
    /// Whether this stops a recording entirely.
    pub fn is_fatal(&self) -> bool {
        !matches!(self, Obstacle::Virtualized { .. })
    }

    /// The exact command or change that fixes it.
    pub fn remedy(&self) -> String {
        match self {
            Obstacle::NotInstalled => "install it: `sudo apt install rr`, or build from \
                 https://github.com/rr-debugger/rr"
                .into(),
            Obstacle::WrongPlatform { found } => format!(
                "nothing — rr is Linux x86-64 only, and this is {found}. Every other analysis in \
                 this product works here; replay does not."
            ),
            Obstacle::Paranoid { current, needed } => format!(
                "`sudo sysctl kernel.perf_event_paranoid={needed}` — it is {current}. rr counts \
                 retired conditional branches to know where it is in a replay, and that is a \
                 performance counter. Add `kernel.perf_event_paranoid={needed}` to \
                 /etc/sysctl.conf to make it survive a reboot."
            ),
            Obstacle::NoPerformanceCounters { because } => format!(
                "{because}. rr cannot replay deterministically without them, so there is no \
                 partial mode to fall back to."
            ),
            Obstacle::Virtualized { hypervisor } => format!(
                "nothing to change, but be aware: this looks like {hypervisor}, and rr has known \
                 incompatibilities in some virtualized environments. If a recording fails \
                 strangely, this is the first thing to suspect."
            ),
        }
    }

    /// What is lost if it is not fixed.
    pub fn costs(&self) -> &'static str {
        match self {
            Obstacle::NotInstalled | Obstacle::WrongPlatform { .. } => {
                "No recording and no replay. Every other analysis still works."
            }
            Obstacle::Paranoid { .. } => {
                "Every recording fails immediately. This is the one that has to be fixed, and it \
                 is the same setting the sampling profiler needs."
            }
            Obstacle::NoPerformanceCounters { .. } => {
                "rr cannot replay deterministically, which is the entire point of it."
            }
            Obstacle::Virtualized { .. } => {
                "Possibly nothing. Recording may work, may be slow, or may fail in ways that \
                 look like a bug in the program being recorded."
            }
        }
    }
}

/// What this machine can do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Readiness {
    pub obstacles: Vec<Obstacle>,
}

impl Readiness {
    pub fn can_record(&self) -> bool {
        !self.obstacles.iter().any(Obstacle::is_fatal)
    }

    /// Only the things that must change.
    pub fn blockers(&self) -> Vec<&Obstacle> {
        self.obstacles.iter().filter(|obstacle| obstacle.is_fatal()).collect()
    }

    /// Everything, with the fix for each.
    ///
    /// Shown before a recording is attempted, which is the whole point:
    /// recording is slow, and finding out afterwards wastes the wait as well
    /// as the attempt.
    pub fn describe(&self) -> String {
        if self.obstacles.is_empty() {
            return "rr is installed and this machine can record.".into();
        }
        self.obstacles
            .iter()
            .map(|obstacle| format!("{} Fix: {}", obstacle.costs(), obstacle.remedy()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// The value `rr` needs.
pub const PARANOID_NEEDED: i32 = 1;

/// Check every constraint.
pub fn readiness() -> Readiness {
    let mut obstacles = Vec::new();

    // Platform first: on anything but Linux x86-64 the rest of the checks are
    // meaningless, and reporting four problems when the answer is "wrong
    // machine" is worse than reporting one.
    if !cfg!(target_os = "linux") || !cfg!(target_arch = "x86_64") {
        obstacles.push(Obstacle::WrongPlatform {
            found: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        });
        return Readiness { obstacles };
    }

    if !on_path("rr") {
        obstacles.push(Obstacle::NotInstalled);
    }

    if let Some(current) = paranoid_level().filter(|level| *level > PARANOID_NEEDED) {
        obstacles.push(Obstacle::Paranoid { current, needed: PARANOID_NEEDED });
    }

    if let Some(because) = performance_counters_unavailable() {
        obstacles.push(Obstacle::NoPerformanceCounters { because });
    }

    if let Some(hypervisor) = hypervisor() {
        obstacles.push(Obstacle::Virtualized { hypervisor });
    }

    Readiness { obstacles }
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
        .unwrap_or(false)
}

fn paranoid_level() -> Option<i32> {
    std::fs::read_to_string("/proc/sys/kernel/perf_event_paranoid").ok()?.trim().parse().ok()
}

/// Whether the CPU's performance counters are usable.
///
/// `/proc/cpuinfo`'s flags are the readable signal: without `pdpe1gb`-era PMU
/// support the counters rr needs are absent, and a kernel built without
/// `CONFIG_PERF_EVENTS` has no `/proc/sys/kernel/perf_event_paranoid` at all.
fn performance_counters_unavailable() -> Option<String> {
    if !std::path::Path::new("/proc/sys/kernel/perf_event_paranoid").exists() {
        return Some(
            "this kernel has no perf_event support at all, so there are no counters to read".into(),
        );
    }

    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    let flags = cpuinfo.lines().find(|line| line.starts_with("flags"))?;
    // `arch_perfmon` is the flag that says the architectural performance
    // monitoring rr depends on exists. Its absence is the clearest signal a
    // CPU cannot host a recording.
    (!flags.contains("arch_perfmon")).then(|| {
        "this CPU does not report `arch_perfmon`, so it has no architectural \
                  performance monitoring"
            .to_string()
    })
}

/// Which hypervisor this looks like, if any.
fn hypervisor() -> Option<String> {
    // The DMI product name is the most reliable signal that does not need
    // root, and it names the hypervisor rather than merely saying "virtual".
    if let Ok(name) = std::fs::read_to_string("/sys/class/dmi/id/product_name") {
        let name = name.trim();
        for known in ["VMware", "VirtualBox", "KVM", "QEMU", "Hyper-V", "Xen", "Parallels"] {
            if name.contains(known) {
                return Some(known.to_string());
            }
        }
    }
    // `hypervisor` in the CPU flags means something is virtualising this,
    // without saying what.
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    cpuinfo
        .lines()
        .find(|line| line.starts_with("flags"))
        .filter(|flags| flags.contains(" hypervisor"))
        .map(|_| "an unidentified hypervisor".to_string())
}

/// The command to record with.
///
/// Returned as arguments rather than a string, because a command shown to a
/// user has to be one they can copy and one we can execute, and building it
/// twice is how those diverge.
pub fn record_command(program: &std::path::Path, arguments: &[String]) -> Vec<String> {
    let mut command = vec![
        "rr".into(),
        "record".into(),
        // Chaos mode reorders thread scheduling to shake out races. Off by
        // default: it makes a recording less representative of what actually
        // happened, which is the opposite of what a recording is for.
        "--".into(),
        program.display().to_string(),
    ];
    command.extend(arguments.iter().cloned());
    command
}

/// The command to replay with, exposing a GDB remote stub on `port`.
///
/// `§5`: "`rr replay -s <port>` exposes a GDB remote stub, so the practical
/// control path is GDB/MI".
pub fn replay_command(port: u16) -> Vec<String> {
    vec![
        "rr".into(),
        "replay".into(),
        "-s".into(),
        port.to_string(),
        // Do not start a debugger of its own: we are the debugger.
        "--no-start".into(),
    ]
}
