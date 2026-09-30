//! GDB/MI, the control path for a replay (`F4.2`, `F4.3`, `TOOLING §5`).
//!
//! `§5` withdraws an earlier suggestion to drive LLDB through its Python API
//! and recommends DAP — except for `rr`, which speaks GDB's protocol, so
//! GDB/MI it is. "Uglier than DAP but battle-tested."
//!
//! MI is a line protocol with a peculiar record syntax: a result record starts
//! with `^`, an asynchronous one with `*` or `=`, console output with `~`, and
//! a token may prefix any of them to match a reply to a request. The values
//! are a nested `key=value` grammar with `{}` for tuples and `[]` for lists,
//! and it is *not* JSON — strings are C-escaped and keys repeat inside a list.
//!
//! What is parsed here is what a replay needs and no more. A full MI value
//! parser is a larger thing than this uses, and the parts skipped are skipped
//! deliberately rather than forgotten.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What kind of record a line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordKind {
    /// `^done`, `^running`, `^error` — the reply to a command.
    Result,
    /// `*stopped`, `*running` — the program's state changed.
    ExecAsync,
    /// `=thread-created` and friends — gdb telling us about itself.
    NotifyAsync,
    /// `~`, `@`, `&` — text for a human.
    Stream,
}

/// One record from gdb.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub kind: RecordKind,
    /// `done`, `error`, `stopped`, and so on.
    pub class: String,
    /// The token the command carried, where it had one. This is how a reply is
    /// matched to a request, and without it a client that has two commands in
    /// flight attributes one's answer to the other.
    pub token: Option<u32>,
    pub fields: BTreeMap<String, String>,
}

impl Record {
    /// Whether this is an error reply.
    pub fn is_error(&self) -> bool {
        self.kind == RecordKind::Result && self.class == "error"
    }

    /// The error message, where there is one.
    pub fn error(&self) -> Option<&str> {
        self.is_error().then(|| self.fields.get("msg").map(String::as_str)).flatten()
    }

    /// Whether the program has stopped.
    pub fn is_stop(&self) -> bool {
        self.kind == RecordKind::ExecAsync && self.class == "stopped"
    }

    /// Why it stopped.
    pub fn stop_reason(&self) -> Option<&str> {
        self.is_stop().then(|| self.fields.get("reason").map(String::as_str)).flatten()
    }
}

/// Parse one line of MI.
///
/// `None` for `(gdb)`, the prompt, and for a blank line — neither is a record,
/// and treating the prompt as one produces a record with an empty class that
/// a caller has to filter anyway.
pub fn parse(line: &str) -> Option<Record> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.is_empty() || line == "(gdb)" || line == "(gdb) " {
        return None;
    }

    // An optional numeric token precedes the record character.
    let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
    let rest = &line[digits.len()..];
    let token = (!digits.is_empty()).then(|| digits.parse().ok()).flatten();

    let mut characters = rest.chars();
    let marker = characters.next()?;
    let body = characters.as_str();

    let kind = match marker {
        '^' => RecordKind::Result,
        '*' => RecordKind::ExecAsync,
        '=' => RecordKind::NotifyAsync,
        '~' | '@' | '&' => {
            // Stream output is a single C-quoted string with no class.
            return Some(Record {
                kind: RecordKind::Stream,
                class: String::new(),
                token,
                fields: BTreeMap::from([("text".to_string(), unquote(body))]),
            });
        }
        _ => return None,
    };

    let (class, fields) = match body.split_once(',') {
        Some((class, rest)) => (class.to_string(), parse_fields(rest)),
        None => (body.to_string(), BTreeMap::new()),
    };

    Some(Record { kind, class, token, fields })
}

/// Parse a comma-separated `key=value` list.
///
/// Values may be quoted strings, `{tuples}` or `[lists]`. Nested values are
/// kept as their raw text rather than parsed into a tree: what a replay needs
/// from them is a handful of scalars, and a half-built tree is more misleading
/// than an honest string.
fn parse_fields(input: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    let bytes: Vec<char> = input.chars().collect();
    let mut index = 0;

    while index < bytes.len() {
        // The key, up to `=`.
        let start = index;
        while index < bytes.len() && bytes[index] != '=' {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let key: String = bytes[start..index].iter().collect();
        index += 1;

        // The value, which ends at a comma that is not inside anything.
        let start = index;
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escaped = false;
        while index < bytes.len() {
            let character = bytes[index];
            if escaped {
                escaped = false;
            } else if character == '\\' && in_string {
                escaped = true;
            } else if character == '"' {
                in_string = !in_string;
            } else if !in_string {
                match character {
                    '{' | '[' => depth += 1,
                    '}' | ']' => depth -= 1,
                    // A comma at depth zero ends this value. Without the depth
                    // check a `frame={...,...}` splits in the middle.
                    ',' if depth == 0 => break,
                    _ => {}
                }
            }
            index += 1;
        }
        let raw: String = bytes[start..index].iter().collect();
        fields.insert(key.trim().to_string(), unquote(&raw));
        index += 1;
    }
    fields
}

/// Strip surrounding quotes and undo C escaping.
fn unquote(value: &str) -> String {
    let value = value.trim();
    let Some(inner) = value.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) else {
        return value.to_string();
    };
    let mut out = String::with_capacity(inner.len());
    let mut characters = inner.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            // An escape we do not know is kept as written rather than dropped:
            // dropping it silently changes a path or a message.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// A direction to execute in.
///
/// The whole point of a replay, and the reason the tool exists: forwards
/// answers "what happened next", backwards answers "how did it get like this".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Forward,
    Reverse,
}

/// What to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Step {
    /// Run until something stops it.
    Continue,
    /// One source line, stepping into calls.
    Into,
    /// One source line, over calls.
    Over,
    /// Until the current function returns — or, reversed, until it was called.
    Finish,
}

/// The MI command for a step in a direction (`F4.2`).
///
/// Reverse execution is `--reverse` on the ordinary commands rather than a
/// separate set, which is why this is one function rather than two.
pub fn step_command(token: u32, step: Step, direction: Direction) -> String {
    let verb = match step {
        Step::Continue => "exec-continue",
        Step::Into => "exec-step",
        Step::Over => "exec-next",
        Step::Finish => "exec-finish",
    };
    match direction {
        Direction::Forward => format!("{token}-{verb}"),
        Direction::Reverse => format!("{token}-{verb} --reverse"),
    }
}

/// How many hardware watchpoints x86-64 has.
///
/// `F4.3`: "four are available on x86-64 and the interface says so rather than
/// silently single-stepping". The difference is not cosmetic — a software
/// watchpoint single-steps the entire program and turns a question that takes
/// a second into one that takes an hour.
pub const HARDWARE_WATCHPOINTS: usize = 4;

/// A request for a watchpoint, and whether the hardware can serve it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Watchpoint {
    /// The hardware will handle it.
    Hardware { expression: String },
    /// There are no debug registers left, so gdb would single-step.
    ///
    /// Returned rather than set: `F4.3` says the interface says so rather than
    /// silently single-stepping, and silently is exactly what gdb does.
    WouldSingleStep { expression: String, already_set: usize },
}

impl Watchpoint {
    pub fn is_hardware(&self) -> bool {
        matches!(self, Watchpoint::Hardware { .. })
    }

    /// The MI command, for a watchpoint the hardware can serve.
    pub fn command(&self, token: u32) -> Option<String> {
        match self {
            Watchpoint::Hardware { expression } => {
                Some(format!("{token}-break-watch -a {expression}"))
            }
            Watchpoint::WouldSingleStep { .. } => None,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Watchpoint::Hardware { expression } => {
                format!("watching {expression} in hardware")
            }
            Watchpoint::WouldSingleStep { expression, already_set } => format!(
                "{expression} cannot be watched in hardware: x86-64 has {HARDWARE_WATCHPOINTS} \
                 debug registers and {already_set} are in use. gdb would fall back to \
                 single-stepping the whole program, which turns a question that takes a second \
                 into one that takes an hour — so this is being refused rather than done quietly."
            ),
        }
    }
}

/// Ask for a watchpoint, given how many are already set.
pub fn watch(expression: impl Into<String>, already_set: usize) -> Watchpoint {
    let expression = expression.into();
    if already_set < HARDWARE_WATCHPOINTS {
        Watchpoint::Hardware { expression }
    } else {
        Watchpoint::WouldSingleStep { expression, already_set }
    }
}

/// Where a replay is, as a count of events.
///
/// `rr` numbers execution deterministically, which is what makes scrubbing a
/// timeline possible at all: a position is a number, and going back to it
/// gives exactly the same program state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Position(pub u64);

/// The command to jump to a position (`F4.4`).
pub fn seek_command(token: u32, position: Position) -> String {
    format!("{token}-interpreter-exec console \"run {}\"", position.0)
}
