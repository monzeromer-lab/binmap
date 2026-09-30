//! Tests for the headless harness.
//!
//! In their own file rather than a `mod tests` at the foot of `main.rs`:
//! every new subcommand was appended after that module, which puts an item
//! after a test module and trips clippy. Three times was enough.
use super::*;
use binmap_core::event::RunId;

fn run() -> RunId {
    RunId("test-0001".into())
}

#[test]
fn a_fresh_printer_has_no_failure_to_report() {
    assert_eq!(Printer::new().failure(), None);
}

#[test]
fn a_failed_event_is_remembered_so_the_caller_can_exit_nonzero() {
    // The regression this guards: a sweep that failed before building
    // anything printed an empty frontier and exited 0, which in CI reads
    // exactly like success.
    let printer = Printer::new();
    printer.emit(EngineEvent::Failed {
        run: run(),
        error: "another binmap run is already working here".into(),
    });

    let failure = printer.failure().expect("a Failed event must be recorded");
    assert!(failure.contains("already working"), "the reason is kept verbatim: {failure}");
}

#[test]
fn a_failure_is_also_terminal_so_a_waiting_caller_is_released() {
    // Recording the reason is no use if `wait` never returns.
    let printer = Printer::new();
    printer.emit(EngineEvent::Failed { run: run(), error: "stopped".into() });
    printer.wait();
    assert!(printer.failure().is_some());
}

#[test]
fn a_run_that_finished_reports_no_failure() {
    let printer = Printer::new();
    printer
        .emit(EngineEvent::Finished { run: run(), summary: "96 configurations measured".into() });
    printer.wait();
    assert_eq!(printer.failure(), None, "a finished run has not failed");
}
