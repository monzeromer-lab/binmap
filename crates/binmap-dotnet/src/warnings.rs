//! Trim and AOT warnings as findings (`TOOLING-DOTNET §5`).
//!
//! `§5` calls this "the differentiated, deterministic finding type that
//! justifies the backend", and the reason is that it has no analogue
//! elsewhere: when trimming or compiling AOT, the toolchain identifies code it
//! **cannot statically analyse** — reflection over types that might be
//! trimmed, dynamic code generation AOT cannot support. Each such warning has
//! a source location, a deterministic origin and a known class of remedy.
//!
//! "You cannot trim this because of reflection at `Foo.cs:42`, and here is the
//! annotation or the source-generator alternative" is well-grounded and
//! actionable, and needs no model to produce.
//!
//! One setting matters enough to name: by default these are **aggregated per
//! assembly**, which collapses forty sites into one line and makes them
//! unactionable. `<TrimmerSingleWarn>false</TrimmerSingleWarn>` turns that
//! off, and a project without it gets told so rather than getting a short and
//! useless list.

use serde::{Deserialize, Serialize};

/// What kind of problem a warning describes.
/// Ordered so `Aot > Trim`: one breaks the build's output and the other makes
/// it bigger, and a list is sorted by which matters more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Kind {
    /// `IL2xxx`: trimming cannot prove this is safe.
    Trim,
    /// `IL3xxx`: AOT cannot compile this at all.
    ///
    /// Worse than a trim warning, and worth separating: a trim warning means
    /// the output may be larger or may break at runtime, and an AOT one means
    /// the build does not work.
    Aot,
}

impl Kind {
    /// Which class a warning code belongs to.
    pub fn of(code: &str) -> Option<Self> {
        let number = code.strip_prefix("IL")?;
        match number.chars().next()? {
            '2' => Some(Kind::Trim),
            '3' => Some(Kind::Aot),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Trim => "trim",
            Kind::Aot => "AOT",
        }
    }

    /// What this class of warning costs if ignored.
    pub fn costs(self) -> &'static str {
        match self {
            Kind::Trim => {
                "Trimming cannot prove this is safe, so it either keeps more than it needs to or \
                 removes something reflection will look for at runtime. The second failure \
                 happens in production, not at build time."
            }
            Kind::Aot => {
                "AOT cannot compile this. Unlike a trim warning it is not a size question — the \
                 published binary will fail at the point this runs."
            }
        }
    }
}

/// One warning, as a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    pub code: String,
    pub kind: Kind,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    /// The compiler's own message, verbatim.
    pub message: String,
    /// The member it is about, where the message names one.
    pub member: Option<String>,
}

impl Warning {
    /// The class of remedy, from the code.
    ///
    /// Named classes rather than a generated fix: the remedies here are a
    /// small, well-known set, and a reader who recognises the class knows what
    /// to do far faster than they can read a paragraph.
    pub fn remedy(&self) -> &'static str {
        match self.code.as_str() {
            "IL2026" => {
                "The member is annotated `RequiresUnreferencedCode`. Either annotate the caller \
                 the same way and push the decision up, or replace the reflection with a source \
                 generator, which is the route that actually removes the problem."
            }
            "IL2057" | "IL2072" | "IL2075" => {
                "A type is being looked up or reflected over in a way trimming cannot follow. \
                 `DynamicallyAccessedMembers` on the parameter tells the trimmer what to keep."
            }
            "IL3050" => {
                "The member is annotated `RequiresDynamicCode`, which AOT cannot provide. There \
                 is usually a source-generated alternative — `JsonSerializerContext` for \
                 System.Text.Json is the common one."
            }
            "IL3053" => {
                "AOT analysis found a pattern it cannot compile. The message names it; there is \
                 rarely an annotation that helps, so this normally means changing the approach."
            }
            _ => match self.kind {
                Kind::Trim => {
                    "Trimming cannot follow this statically. `DynamicallyAccessedMembers` or a \
                     source generator are the two routes."
                }
                Kind::Aot => {
                    "AOT cannot compile this. A source-generated alternative is the usual \
                     replacement."
                }
            },
        }
    }

    /// The line a findings list shows.
    pub fn describe(&self) -> String {
        let location = match (&self.file, self.line) {
            (Some(file), Some(line)) => {
                let short = file.rsplit(['/', '\\']).next().unwrap_or(file);
                format!("{short}:{line}")
            }
            (Some(file), None) => file.clone(),
            _ => "<no location>".to_string(),
        };
        format!("{} [{}] at {location}: {}", self.code, self.kind.label(), self.message)
    }
}

/// Everything the build said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warnings {
    /// AOT warnings first, then trim: one breaks the build's output and the
    /// other makes it bigger.
    pub warnings: Vec<Warning>,
    /// Whether the warnings look aggregated per assembly.
    ///
    /// `§5`: the default collapses every site in an assembly into one line.
    /// A project that has not turned it off gets a short, useless list, and
    /// saying so is more useful than showing it.
    pub looks_aggregated: bool,
}

impl Warnings {
    pub fn by_kind(&self, kind: Kind) -> Vec<&Warning> {
        self.warnings.iter().filter(|warning| warning.kind == kind).collect()
    }

    /// Whether anything here stops the build working at all.
    pub fn blocks_aot(&self) -> bool {
        self.warnings.iter().any(|warning| warning.kind == Kind::Aot)
    }

    pub fn describe(&self) -> String {
        let aot = self.by_kind(Kind::Aot).len();
        let trim = self.by_kind(Kind::Trim).len();
        let mut line = format!("{aot} AOT warnings and {trim} trim warnings");
        if self.looks_aggregated {
            line.push_str(
                ". These look aggregated per assembly, which collapses every site into one \
                 line. Set `<TrimmerSingleWarn>false</TrimmerSingleWarn>` to get the sites.",
            );
        }
        line
    }
}

/// Read warnings out of MSBuild's output.
///
/// The format is `path(line,col): warning ILxxxx: message`, which MSBuild has
/// emitted unchanged for long enough to rely on — and a line that does not
/// match is skipped rather than guessed at, because a build log contains
/// plenty that is not a warning.
pub fn parse(output: &str) -> Warnings {
    let mut warnings = Vec::new();

    for line in output.lines() {
        let Some((location, rest)) = line.split_once(": warning ") else { continue };
        let Some((code, message)) = rest.split_once(':') else { continue };
        let code = code.trim();
        let Some(kind) = Kind::of(code) else { continue };

        // `path(line,col)` — and the parenthesis is optional, because a
        // warning about a whole assembly has no line.
        let (file, line_number, column) = match location.rsplit_once('(') {
            Some((path, position)) => {
                let position = position.trim_end_matches(')');
                let (line_number, column) = match position.split_once(',') {
                    Some((line_number, column)) => (line_number.parse().ok(), column.parse().ok()),
                    None => (position.parse().ok(), None),
                };
                (Some(path.trim().to_string()), line_number, column)
            }
            None => (Some(location.trim().to_string()).filter(|p| !p.is_empty()), None, None),
        };

        let message = message.trim().to_string();
        warnings.push(Warning {
            code: code.to_string(),
            kind,
            file,
            line: line_number,
            column,
            member: member_in(&message),
            message,
        });
    }

    // A warning with no line, where others have them, is the aggregated form.
    let without_lines = warnings.iter().filter(|warning| warning.line.is_none()).count();
    let looks_aggregated = !warnings.is_empty() && without_lines == warnings.len();

    // AOT first: one breaks the output, the other makes it bigger.
    warnings.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .reverse()
            .then(left.file.cmp(&right.file))
            .then(left.line.cmp(&right.line))
    });

    Warnings { warnings, looks_aggregated }
}

/// The member a warning is about, where the message names one.
///
/// ILC quotes it, which is the only reliable marker — the messages themselves
/// are prose and change between versions.
fn member_in(message: &str) -> Option<String> {
    let start = message.find('\'')?;
    let rest = &message[start + 1..];
    let end = rest.find('\'')?;
    let member = &rest[..end];
    (!member.is_empty()).then(|| member.to_string())
}
