#!/usr/bin/env bash
# Capture the core dumps the crash-analysis tests run against.
#
# The cores are not committed: they are megabytes each and only meaningful
# beside the exact binary that produced them, so the pair is regenerated rather
# than stored. The tests skip when they are absent, so this is a prerequisite
# for those tests and not for the suite.
#
# gdb rather than a crashing process, because `kernel.yama.ptrace_scope` stops
# a process attaching to a non-descendant, and this machine's `core_pattern`
# pipes to apport with `ulimit -c 0`. Running the program *under* gdb sidesteps
# both without needing root.
set -euo pipefail

cd "$(dirname "$0")"
mkdir -p cores

command -v gdb >/dev/null || { echo "gdb is required to capture cores" >&2; exit 1; }

echo "building the crasher"
cargo build --release

# A genuine SIGSEGV: gdb stops at the fault, so the core is taken with the
# faulting instruction still current.
echo "capturing segv.core"
gdb --batch -ex run -ex 'generate-core-file cores/segv.core' \
    --args ./target/release/crasher segv >/dev/null 2>&1 || true

# A panic unwinds and exits cleanly, so there is no process left to dump.
# Built with `panic = "abort"` it raises SIGABRT instead, which gdb stops on —
# and that path goes through libc, which is what makes it the test that forced
# per-module CFI.
echo "capturing panic.core"
RUSTFLAGS="-C panic=abort" cargo build --release --target-dir target/abort
cp target/abort/release/crasher cores/crasher-abort
gdb --batch -ex run -ex 'generate-core-file cores/panic.core' \
    --args ./cores/crasher-abort panic >/dev/null 2>&1 || true

ls -la cores/
