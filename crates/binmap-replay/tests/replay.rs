//! Replay debugging (`F4.1`–`F4.4`).
//!
//! The MI records here are the shapes gdb actually emits. `rr` is not
//! installed on this machine — which is itself one of the conditions the
//! preflight exists to report, so that part is tested against real conditions
//! rather than imagined ones.

use binmap_replay::gdbmi::{
    Direction, HARDWARE_WATCHPOINTS, Position, RecordKind, Step, parse, seek_command, step_command,
    watch,
};
use binmap_replay::preflight::{
    Obstacle, PARANOID_NEEDED, Readiness, readiness, record_command, replay_command,
};

// --- the preflight (F4.1) ---------------------------------------------------

#[test]
fn this_machines_constraints_are_reported_before_anything_is_attempted() {
    // F4.1: "stated before a user's first failed attempt rather than after",
    // because the failure arrives at the end of a slow recording.
    let state = readiness();
    let described = state.describe();
    assert!(!described.is_empty());

    for obstacle in &state.obstacles {
        assert!(!obstacle.remedy().is_empty(), "{obstacle:?} offers no fix");
        assert!(!obstacle.costs().is_empty(), "{obstacle:?} does not say what is lost");
    }
}

#[test]
fn a_paranoid_kernel_explains_why_rr_needs_the_counter() {
    // Someone told to change a kernel setting deserves to know what it is for.
    let obstacle = Obstacle::Paranoid { current: 4, needed: PARANOID_NEEDED };
    let remedy = obstacle.remedy();

    assert!(remedy.contains("sysctl kernel.perf_event_paranoid=1"), "{remedy}");
    assert!(remedy.contains("it is 4"), "{remedy}");
    assert!(
        remedy.contains("retired conditional branches"),
        "it says what the counter is for: {remedy}"
    );
    assert!(obstacle.is_fatal());
}

#[test]
fn a_virtualized_machine_is_warned_rather_than_refused() {
    // Some virtualized environments work and some do not, and refusing
    // outright would stop people whose setup is fine.
    let obstacle = Obstacle::Virtualized { hypervisor: "KVM".into() };
    assert!(!obstacle.is_fatal());
    assert!(obstacle.remedy().contains("KVM"), "{}", obstacle.remedy());
    assert!(obstacle.remedy().contains("first thing to suspect"), "{}", obstacle.remedy());

    let survivable = Readiness { obstacles: vec![obstacle] };
    assert!(survivable.can_record(), "a warning does not stop a recording");
    assert!(survivable.blockers().is_empty());
}

#[test]
fn the_wrong_platform_says_what_still_works() {
    // Every other analysis in the product works on any platform. Saying so
    // stops "replay is unavailable" reading as "this tool is unavailable".
    let obstacle = Obstacle::WrongPlatform { found: "macos aarch64".into() };
    assert!(obstacle.is_fatal());
    assert!(obstacle.remedy().contains("macos aarch64"), "{}", obstacle.remedy());
    assert!(obstacle.costs().contains("Every other analysis still works"));
}

#[test]
fn a_machine_with_nothing_wrong_says_so() {
    let clean = Readiness { obstacles: Vec::new() };
    assert!(clean.can_record());
    assert!(clean.describe().contains("can record"), "{}", clean.describe());
}

#[test]
fn the_record_and_replay_commands_are_built_once() {
    // A command shown to a user has to be one they can copy and one we can
    // execute; building it twice is how those diverge.
    let record = record_command(std::path::Path::new("./app"), &["--flag".into()]);
    assert_eq!(record[0], "rr");
    assert!(record.contains(&"record".to_string()));
    assert_eq!(record.last(), Some(&"--flag".to_string()));

    // §5: `rr replay -s <port>` exposes a GDB remote stub.
    let replay = replay_command(50505);
    assert!(replay.contains(&"-s".to_string()));
    assert!(replay.contains(&"50505".to_string()));
    assert!(
        replay.contains(&"--no-start".to_string()),
        "we are the debugger, so rr must not start one"
    );
}

// --- MI parsing -------------------------------------------------------------

#[test]
fn a_result_record_is_matched_to_its_command_by_token() {
    // Without the token a client with two commands in flight attributes one's
    // answer to the other.
    let record = parse("42^done,value=\"7\"").expect("a result record");
    assert_eq!(record.kind, RecordKind::Result);
    assert_eq!(record.class, "done");
    assert_eq!(record.token, Some(42));
    assert_eq!(record.fields.get("value").map(String::as_str), Some("7"));
}

#[test]
fn an_error_carries_its_message() {
    let record = parse("7^error,msg=\"No symbol \\\"nope\\\" in current context.\"").unwrap();
    assert!(record.is_error());
    assert_eq!(record.error(), Some("No symbol \"nope\" in current context."));
}

#[test]
fn a_stop_says_why_it_stopped() {
    let record = parse(
        "*stopped,reason=\"breakpoint-hit\",disp=\"keep\",bkptno=\"1\",\
         frame={addr=\"0x1234\",func=\"main\",file=\"main.rs\",line=\"42\"},thread-id=\"1\"",
    )
    .expect("an exec-async record");

    assert_eq!(record.kind, RecordKind::ExecAsync);
    assert!(record.is_stop());
    assert_eq!(record.stop_reason(), Some("breakpoint-hit"));
    assert_eq!(record.fields.get("thread-id").map(String::as_str), Some("1"));
}

#[test]
fn a_nested_tuple_does_not_split_at_its_own_commas() {
    // Without tracking depth, `frame={addr=...,func=...}` splits in the middle
    // and every field after it is misread.
    let record = parse(
        "*stopped,reason=\"end-stepping-range\",frame={addr=\"0x1\",func=\"f\",args=[]},\
         thread-id=\"1\"",
    )
    .unwrap();

    assert_eq!(record.stop_reason(), Some("end-stepping-range"));
    assert_eq!(record.fields.get("thread-id").map(String::as_str), Some("1"));
    assert!(record.fields.get("frame").is_some_and(|frame| frame.contains("func=")));
}

#[test]
fn stream_output_is_unquoted_and_kept_as_text() {
    let record = parse("~\"Breakpoint 1 at 0x1234: file main.rs, line 42.\\n\"").unwrap();
    assert_eq!(record.kind, RecordKind::Stream);
    assert!(record.fields["text"].ends_with('\n'), "the escape is undone");
    assert!(record.fields["text"].contains("main.rs"));
}

#[test]
fn an_escape_this_parser_does_not_know_is_kept_rather_than_dropped() {
    // Dropping it silently changes a path or a message.
    let record = parse("~\"a\\qb\"").unwrap();
    assert_eq!(record.fields["text"], "a\\qb");
}

#[test]
fn the_prompt_and_blank_lines_are_not_records() {
    // Treating the prompt as one produces a record with an empty class that
    // every caller has to filter anyway.
    assert!(parse("(gdb)").is_none());
    assert!(parse("(gdb) ").is_none());
    assert!(parse("").is_none());
    assert!(parse("   \n").is_none());
}

#[test]
fn a_record_with_no_fields_is_still_a_record() {
    let record = parse("^running").expect("a bare result");
    assert_eq!(record.class, "running");
    assert!(record.fields.is_empty());
    assert_eq!(record.token, None);
}

#[test]
fn a_notification_is_told_apart_from_an_execution_change() {
    // `=` is gdb talking about itself and `*` is the program moving. A client
    // that treats them alike reports a thread being created as the program
    // stopping.
    let notify = parse("=thread-group-added,id=\"i1\"").unwrap();
    assert_eq!(notify.kind, RecordKind::NotifyAsync);
    assert!(!notify.is_stop());
}

// --- reverse execution (F4.2) ----------------------------------------------

#[test]
fn every_step_has_a_reverse() {
    // The whole point of a replay: forwards answers "what happened next",
    // backwards answers "how did it get like this".
    for step in [Step::Continue, Step::Into, Step::Over, Step::Finish] {
        let forward = step_command(1, step, Direction::Forward);
        let reverse = step_command(1, step, Direction::Reverse);

        assert!(!forward.contains("--reverse"), "{forward}");
        assert!(reverse.contains("--reverse"), "{reverse}");
        // Reverse is a flag on the ordinary command, not a separate one.
        assert!(reverse.starts_with(&forward), "{reverse} should extend {forward}");
    }
}

#[test]
fn the_commands_are_the_ones_gdb_actually_implements() {
    assert_eq!(step_command(1, Step::Continue, Direction::Forward), "1-exec-continue");
    assert_eq!(step_command(2, Step::Into, Direction::Reverse), "2-exec-step --reverse");
    assert_eq!(step_command(3, Step::Over, Direction::Reverse), "3-exec-next --reverse");
    assert_eq!(step_command(4, Step::Finish, Direction::Reverse), "4-exec-finish --reverse");
}

#[test]
fn a_position_can_be_returned_to() {
    // rr numbers execution deterministically, which is what makes scrubbing a
    // timeline possible: a position is a number, and going back to it gives
    // exactly the same state.
    let command = seek_command(9, Position(12_345));
    assert!(command.contains("12345"), "{command}");
    assert!(command.starts_with("9-"), "the token is there too: {command}");
}

// --- watchpoints (F4.3) -----------------------------------------------------

#[test]
fn the_hardware_watchpoints_are_used_until_they_run_out() {
    // x86-64 has four debug registers.
    for already in 0..HARDWARE_WATCHPOINTS {
        let watchpoint = watch("counter", already);
        assert!(watchpoint.is_hardware(), "{already} in use should still leave one");
        assert!(watchpoint.command(1).is_some());
    }
}

#[test]
fn a_fifth_watchpoint_is_refused_rather_than_silently_single_stepped() {
    // F4.3: "four are available on x86-64 and the interface says so rather
    // than silently single-stepping". Silently is exactly what gdb does, and
    // it turns a question that takes a second into one that takes an hour.
    let watchpoint = watch("counter", HARDWARE_WATCHPOINTS);
    assert!(!watchpoint.is_hardware());
    assert!(watchpoint.command(1).is_none(), "no command is issued");

    let described = watchpoint.describe();
    assert!(described.contains("4 debug registers"), "{described}");
    assert!(described.contains("single-stepping"), "{described}");
    assert!(described.contains("takes an hour"), "the cost is concrete: {described}");
}

#[test]
fn a_hardware_watchpoint_asks_for_access_rather_than_write_only() {
    // `-a` watches reads as well as writes. "Who wrote this value" is the
    // question, and "who read it just before it changed" is frequently the
    // answer.
    let watchpoint = watch("state.count", 0);
    let command = watchpoint.command(5).expect("hardware");
    assert!(command.contains("-break-watch"), "{command}");
    assert!(command.contains("-a"), "{command}");
    assert!(command.contains("state.count"), "{command}");
}
