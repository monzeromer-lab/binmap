//! A sampling profiler (`F3.1`).
//!
//! `perf` needs `kernel.perf_event_paranoid ≤ 1` and this machine has 4, so
//! the design's default source is unavailable without root. But a sampling
//! profiler is not a mysterious thing: it is an unwinder run a thousand times
//! a second, and Phase 2 already built the unwinder. This stops the process
//! periodically, reads its registers, walks its stack with the same CFI code
//! the crash analyser uses, and lets it go.
//!
//! **It profiles a process it spawned itself.** `kernel.yama.ptrace_scope` is
//! 1 on most distributions, which permits tracing a descendant and forbids
//! anything else — so attaching to something already running is not available
//! and is not attempted. That is a real limitation, and the honest way to have
//! it is by construction rather than by a runtime failure.
//!
//! The cost of stopping a process to look at it is real and is why `perf`
//! exists: each sample costs two context switches and a few reads. At a
//! hundred samples a second on a program doing real work that is tolerable,
//! and a profile is an approximation anyway — but it is why the sampling rate
//! here is conservative rather than the 999Hz a hardware profiler uses.

use crate::sample::{Profile, Source, Stack};
use binmap_core::error::{Error, Result};
use binmap_crash::memory::Memory;
use binmap_crash::modules::Modules;
use binmap_crash::registers::{PtraceSlot, Registers};
use std::path::Path;
use std::time::{Duration, Instant};

/// A live process's memory, read through `/proc/PID/mem`.
///
/// `/proc/PID/mem` rather than `PTRACE_PEEKDATA`: peek reads a word per
/// syscall, and a stack walk reads dozens per sample. Reading the file is one
/// syscall for the whole span.
pub struct LiveMemory {
    file: std::fs::File,
    /// What the process has mapped, read once per sample from
    /// `/proc/PID/maps`.
    mapped: Vec<(u64, u64)>,
}

impl LiveMemory {
    pub fn attach(pid: i32) -> Result<Self> {
        let file = std::fs::File::open(format!("/proc/{pid}/mem")).map_err(|error| {
            Error::Other(format!("could not read process {pid}'s memory: {error}"))
        })?;
        Ok(Self { file, mapped: read_maps(pid) })
    }

    /// Re-read the mappings. A process that loads a library mid-run changes
    /// them, and a stale table rejects real return addresses.
    pub fn refresh(&mut self, pid: i32) {
        self.mapped = read_maps(pid);
    }
}

impl Memory for LiveMemory {
    fn read(&self, address: u64, length: usize) -> Option<Vec<u8>> {
        use std::os::unix::fs::FileExt;
        let mut buffer = vec![0u8; length];
        // `read_exact_at` rather than seek-then-read: the seek and the read
        // are separate syscalls, and between them the process has moved on.
        self.file.read_exact_at(&mut buffer, address).ok()?;
        Some(buffer)
    }

    fn is_mapped(&self, address: u64) -> bool {
        self.mapped.iter().any(|(start, end)| (*start..*end).contains(&address))
    }
}

/// What a process has mapped, from `/proc/PID/maps`.
fn read_maps(pid: i32) -> Vec<(u64, u64)> {
    let Ok(maps) = std::fs::read_to_string(format!("/proc/{pid}/maps")) else {
        return Vec::new();
    };
    maps.lines()
        .filter_map(|line| {
            let range = line.split_whitespace().next()?;
            let (start, end) = range.split_once('-')?;
            Some((u64::from_str_radix(start, 16).ok()?, u64::from_str_radix(end, 16).ok()?))
        })
        .collect()
}

/// How often to sample, and for how long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    pub interval: Duration,
    pub duration: Duration,
}

impl Default for Plan {
    /// A hundred samples a second for two seconds.
    ///
    /// Conservative on purpose. Each sample stops the process, so the
    /// sampling rate is a tax on the thing being measured — a hardware
    /// profiler can afford 999Hz because it does not stop anything, and this
    /// cannot.
    fn default() -> Self {
        Self { interval: Duration::from_millis(10), duration: Duration::from_secs(2) }
    }
}

impl Plan {
    /// How many samples this asks for.
    pub fn expected_samples(&self) -> u64 {
        (self.duration.as_secs_f64() / self.interval.as_secs_f64()) as u64
    }
}

/// Profile a program by running it.
///
/// Spawns `program` with `arguments`, samples it, and returns what it saw.
/// The process is killed when the plan's duration elapses, so this profiles a
/// *window* of a program rather than a whole run — which is what a profiler of
/// a long-running program does anyway.
pub fn profile(program: &Path, arguments: &[String], plan: Plan) -> Result<(Profile, Modules)> {
    profile_with(program, arguments, &[], plan)
}

/// Profile a program with extra environment variables.
///
/// The environment is how a regression is injected without rebuilding, which
/// matters: the same binary profiled twice is the only way to know a
/// difference came from the change rather than from the compiler.
pub fn profile_with(
    program: &Path,
    arguments: &[String],
    environment: &[(String, String)],
    plan: Plan,
) -> Result<(Profile, Modules)> {
    use std::os::unix::process::CommandExt;

    let mut command = std::process::Command::new(program);
    command.args(arguments);
    for (name, value) in environment {
        command.env(name, value);
    }
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::null());

    // `PTRACE_TRACEME` in the child, before exec. This is what makes us the
    // tracer under yama's descendant rule, rather than attaching afterwards —
    // which `ptrace_scope = 1` forbids.
    unsafe {
        command.pre_exec(|| {
            if ptrace(PTRACE_TRACEME, 0, 0, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let mut child = command
        .spawn()
        .map_err(|error| Error::Other(format!("could not start {}: {error}", program.display())))?;
    let pid = child.id() as i32;

    let result = sample_loop(pid, plan);

    // Whatever happened, do not leave a stopped process behind.
    let _ = kill(pid);
    let _ = child.wait();

    result
}

/// The sampling loop.
fn sample_loop(pid: i32, plan: Plan) -> Result<(Profile, Modules)> {
    // The child stops at exec, which is where its mappings first exist.
    wait_for_stop(pid);

    let mut memory = LiveMemory::attach(pid)?;
    let modules = modules_of(pid);
    let mut profile = Profile::new(Source::Internal, format!("binmap sample pid {pid}"));

    let started = Instant::now();
    let mut since_refresh = 0u32;

    // Let it run.
    if ptrace(PTRACE_CONT, pid, 0, 0) < 0 {
        return Err(Error::Other("could not resume the process to be profiled".into()));
    }

    while started.elapsed() < plan.duration {
        std::thread::sleep(plan.interval);

        // Stop it, look, let it go. A process that has already exited stops
        // being interruptible, which ends the profile rather than failing it.
        if kill_stop(pid) < 0 {
            break;
        }
        if !wait_for_stop(pid) {
            break;
        }

        // Mappings change when a library loads, and a stale table rejects
        // real return addresses — but re-reading `/proc/PID/maps` every
        // sample costs more than it is worth.
        since_refresh += 1;
        if since_refresh >= 50 {
            memory.refresh(pid);
            since_refresh = 0;
        }

        if let Some(stack) = read_registers(pid)
            .and_then(|registers| binmap_crash::unwind::walk(&memory, &modules, &registers).ok())
        {
            let addresses: Vec<u64> =
                stack.frames.iter().map(|frame| frame.runtime_address).collect();
            if !addresses.is_empty() {
                profile.record(Stack { addresses }, 1);
            }
        }

        if ptrace(PTRACE_CONT, pid, 0, 0) < 0 {
            break;
        }
    }

    Ok((profile, modules))
}

/// The modules a live process has mapped.
///
/// Built from `/proc/PID/maps` in the shape `Modules::load` expects, so the
/// same per-module CFI the crash analyser uses applies here unchanged.
fn modules_of(pid: i32) -> Modules {
    let dump = binmap_crash::dump::CoreDump {
        threads: Vec::new(),
        mappings: read_named_maps(pid),
        segments: Vec::new(),
        executable: None,
        auxv_phdr: None,
        auxv_entry: None,
    };
    Modules::load(&dump, None)
}

/// Named file mappings from `/proc/PID/maps`.
fn read_named_maps(pid: i32) -> Vec<binmap_crash::dump::Mapping> {
    let Ok(maps) = std::fs::read_to_string(format!("/proc/{pid}/maps")) else {
        return Vec::new();
    };
    maps.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let range = fields.next()?;
            let _permissions = fields.next()?;
            let offset = fields.next()?;
            let _device = fields.next()?;
            let _inode = fields.next()?;
            let path = fields.next()?;
            if !path.starts_with('/') {
                return None;
            }
            let (start, end) = range.split_once('-')?;
            Some(binmap_crash::dump::Mapping {
                path: path.to_string(),
                start: u64::from_str_radix(start, 16).ok()?,
                end: u64::from_str_radix(end, 16).ok()?,
                file_offset: u64::from_str_radix(offset, 16).ok()?,
            })
        })
        .collect()
}

// --- the small amount of libc this needs ------------------------------------

const PTRACE_TRACEME: i32 = 0;
const PTRACE_CONT: i32 = 7;
const PTRACE_GETREGS: i32 = 12;
const SIGSTOP: i32 = 19;
const SIGKILL: i32 = 9;

unsafe extern "C" {
    #[link_name = "ptrace"]
    fn ptrace_raw(request: i32, pid: i32, address: u64, data: u64) -> i64;
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    #[link_name = "kill"]
    fn kill_raw(pid: i32, signal: i32) -> i32;
}

fn ptrace(request: i32, pid: i32, address: u64, data: u64) -> i64 {
    unsafe { ptrace_raw(request, pid, address, data) }
}

fn kill_stop(pid: i32) -> i32 {
    unsafe { kill_raw(pid, SIGSTOP) }
}

fn kill(pid: i32) -> i32 {
    unsafe { kill_raw(pid, SIGKILL) }
}

/// Wait for the process to stop. `false` when it exited instead.
fn wait_for_stop(pid: i32) -> bool {
    let mut status = 0i32;
    let result = unsafe { waitpid(pid, &mut status, 0) };
    if result < 0 {
        return false;
    }
    // WIFSTOPPED: the low byte is 0x7f when stopped.
    (status & 0xff) == 0x7f
}

/// `PTRACE_GETREGS` into a `user_regs_struct`.
///
/// The layout is the same one `registers.rs` decodes out of a core's
/// `NT_PRSTATUS` note — which is the point: the register numbering trap is
/// solved once, and neither reader indexes the array itself.
fn read_registers(pid: i32) -> Option<Registers> {
    let mut slots = [0u64; PtraceSlot::COUNT];
    let result = ptrace(PTRACE_GETREGS, pid, 0, slots.as_mut_ptr() as u64);
    (result >= 0).then(|| Registers::from_slots(slots.to_vec()))
}
