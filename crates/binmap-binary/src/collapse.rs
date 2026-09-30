//! Collapse strategies for a generic that costs too much (`A1.5`).
//!
//! `F1.3` says *which* generics are worth collapsing. This says *how*, and the
//! two are deliberately separate: the cost is measured, the strategy is a named
//! rule applied to that measurement, and the difference is the difference
//! between `Measured` and `Derived`.
//!
//! The rules are a catalogue rather than a model's suggestion, and that is the
//! point of `§2.4`. Every strategy here is one a Rust programmer would
//! recognise, each names the condition under which it applies, and each states
//! what it costs — because all of them cost something, and a proposal that
//! only shows the saving is a proposal that will be regretted.
//!
//! **What this does not do is write the patch.** Collapsing a generic is a
//! source transformation, and doing it correctly needs the source parsed, not
//! pattern-matched. Emitting a diff built from a guess at what the signature
//! looks like would produce something that does not compile, attached to a
//! measured saving that looks authoritative. So a strategy carries a worked
//! sketch and the reader applies it. `§6`'s gates verify what comes back, which
//! is the half that makes the advice safe to act on.

use binmap_core::attribution::Monomorphization;
use serde::{Deserialize, Serialize};

/// A named way to make a generic cost less.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Strategy {
    /// Split the function in two: a thin generic shell that converts, and a
    /// non-generic body that does the work. Only the shell is duplicated.
    ///
    /// The standard library does this everywhere — `File::open` is generic over
    /// `AsRef<Path>` and immediately calls a non-generic inner function. It is
    /// the first thing to reach for because it changes no call site.
    Outline,
    /// Take `&dyn Trait` instead of `impl Trait`, so there is one copy.
    ///
    /// Costs an indirect call, which matters in a hot loop and does not
    /// elsewhere. The honest version of "just use dyn".
    Dynamic,
    /// The parameter is never used in a way that needs its type.
    ///
    /// Usually a parameter that is only passed through. Removing it is free,
    /// which makes it the one strategy with no trade to weigh.
    UnusedParameter,
    /// Many instantiations over integer or float widths that share a body.
    ///
    /// Collapsing to the widest type costs a conversion and can change
    /// overflow behaviour, which is why this is the one that needs tests more
    /// than the others.
    WidenNumeric,
}

impl Strategy {
    pub fn label(self) -> &'static str {
        match self {
            Strategy::Outline => "outline the non-generic body",
            Strategy::Dynamic => "take a trait object instead",
            Strategy::UnusedParameter => "drop the unused type parameter",
            Strategy::WidenNumeric => "collapse to one numeric width",
        }
    }

    /// The rule name recorded as provenance.
    ///
    /// Named because `Derived` without a rule name is indistinguishable from a
    /// guess.
    pub fn rule(self) -> &'static str {
        match self {
            Strategy::Outline => "collapse-strategy-outline",
            Strategy::Dynamic => "collapse-strategy-dynamic",
            Strategy::UnusedParameter => "collapse-strategy-unused-parameter",
            Strategy::WidenNumeric => "collapse-strategy-widen-numeric",
        }
    }

    /// What it costs. Never empty: every one of these trades something.
    pub fn cost(self) -> &'static str {
        match self {
            Strategy::Outline => {
                "The shell is still instantiated per type, so the saving is the body rather than \
                 the whole function. Nothing else changes: callers are untouched and the body \
                 stays statically dispatched."
            }
            Strategy::Dynamic => {
                "One indirect call per invocation, and the compiler can no longer inline through \
                 it. Worth measuring rather than assuming — in a hot loop this can cost more \
                 than the bytes it saves."
            }
            Strategy::UnusedParameter => {
                "Nothing, if it really is unused. Check for a `PhantomData` or a trait bound that \
                 is load-bearing for inference before removing it."
            }
            Strategy::WidenNumeric => {
                "A conversion at each call site, and arithmetic that overflowed at the narrow \
                 width will not at the wide one. The behaviour change is the risk, not the \
                 conversion."
            }
        }
    }

    /// A worked sketch of the transformation.
    pub fn sketch(self) -> &'static str {
        match self {
            Strategy::Outline => {
                "fn read(path: impl AsRef<Path>) -> Result<String> {\n\
                 \x20   read_inner(path.as_ref())          // the shell: one line, per type\n\
                 }\n\
                 fn read_inner(path: &Path) -> Result<String> {\n\
                 \x20   /* the body, instantiated once */\n\
                 }"
            }
            Strategy::Dynamic => {
                "-fn render(target: &mut impl Write) -> Result<()>\n\
                 +fn render(target: &mut dyn Write) -> Result<()>"
            }
            Strategy::UnusedParameter => {
                "-fn parse<T: Config>(input: &str) -> Ast\n\
                 +fn parse(input: &str) -> Ast"
            }
            Strategy::WidenNumeric => {
                "-fn clamp<T: Into<i64>>(value: T, limit: T) -> T\n\
                 +fn clamp(value: i64, limit: i64) -> i64"
            }
        }
    }
}

/// One strategy offered for one generic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub strategy: Strategy,
    /// What suggested it, in terms of what was measured.
    pub because: String,
    /// An upper bound on the saving, and labelled as one everywhere it is
    /// shown. The largest instantiation has to stay.
    pub saves_at_most: u64,
    /// How sure we are that this strategy applies at all — not how sure we are
    /// of the number, which is measured.
    pub applicability: Applicability,
}

/// How confidently a strategy can be offered from the symbol table alone.
///
/// The symbol table says what was instantiated, not what the source looks like.
/// Some strategies follow from the instantiation pattern; others need the
/// signature, which is not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Applicability {
    /// The instantiation pattern alone makes this the right move.
    Likely,
    /// Plausible, but the signature decides. Shown with the question the
    /// reader has to answer.
    NeedsTheSignature,
}

impl Applicability {
    pub fn label(self) -> &'static str {
        match self {
            Applicability::Likely => "likely",
            Applicability::NeedsTheSignature => "check the signature",
        }
    }
}

/// Below this, collapsing is not worth a person's attention.
///
/// A kilobyte is roughly the point at which a saving survives being rounded in
/// a report. Offering a strategy that saves two hundred bytes trains the reader
/// to ignore the list.
pub const WORTH_REPORTING: u64 = 1024;

/// What could be done about this generic, best first.
///
/// Empty when nothing is worth doing, which is the common case and is meant to
/// be: a list that always has something in it is a list nobody reads.
pub fn strategies_for(monomorphization: &Monomorphization) -> Vec<Candidate> {
    let saving = monomorphization.collapsible_bytes();
    if saving < WORTH_REPORTING || monomorphization.instantiations < 2 {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    let arguments: Vec<&str> =
        monomorphization.arguments.iter().map(|(argument, _)| argument.as_str()).collect();

    // Outlining is offered whenever a generic is instantiated more than a
    // couple of times, because it is the one strategy that changes no call site
    // and so is always worth considering.
    candidates.push(Candidate {
        strategy: Strategy::Outline,
        because: format!(
            "{} instantiations share a body. Outlining duplicates only the conversion.",
            monomorphization.instantiations
        ),
        saves_at_most: saving,
        applicability: Applicability::Likely,
    });

    // Every argument a reference or a smart pointer suggests the body never
    // needed the concrete type — the classic `impl AsRef` / `impl Into` shape.
    if arguments.iter().all(|argument| looks_like_a_handle(argument)) {
        candidates.push(Candidate {
            strategy: Strategy::Dynamic,
            because: format!(
                "every instantiation is a reference or pointer type ({}), so the body probably \
                 never needed the concrete type",
                sample(&arguments)
            ),
            saves_at_most: saving,
            applicability: Applicability::NeedsTheSignature,
        });
    }

    // All-numeric arguments over a shared body is the widen case.
    if arguments.len() >= 2 && arguments.iter().all(|argument| is_numeric(argument)) {
        candidates.push(Candidate {
            strategy: Strategy::WidenNumeric,
            because: format!(
                "every instantiation is a numeric width ({}), which usually means one body \
                 compiled several times",
                sample(&arguments)
            ),
            saves_at_most: saving,
            applicability: Applicability::NeedsTheSignature,
        });
    }

    // Instantiations that are all the same size to the byte did not depend on
    // their type parameter at all — the bytes say so.
    if monomorphization.instantiations >= 3
        && every_instantiation_is_the_same_size(monomorphization)
    {
        candidates.push(Candidate {
            strategy: Strategy::UnusedParameter,
            because: format!(
                "all {} instantiations compiled to exactly the same size, so the body does not \
                 appear to depend on the parameter",
                monomorphization.instantiations
            ),
            applicability: Applicability::Likely,
            saves_at_most: saving,
        });
    }

    // Best first: what is likely before what needs checking, and within each,
    // the larger saving.
    candidates.sort_by(|left, right| {
        left.applicability
            .cmp(&right.applicability)
            .then(right.saves_at_most.cmp(&left.saves_at_most))
    });
    candidates
}

/// Whether every instantiation compiled to the same number of bytes.
///
/// A strong signal, and one only the symbol table can give: if the parameter
/// changed what the body did, the sizes would differ.
fn every_instantiation_is_the_same_size(monomorphization: &Monomorphization) -> bool {
    let mut sizes = monomorphization.arguments.iter().map(|(_, bytes)| *bytes);
    let Some(first) = sizes.next() else { return false };
    // Zero-byte symbols are not evidence of anything.
    first > 0 && sizes.all(|bytes| bytes == first)
}

fn looks_like_a_handle(argument: &str) -> bool {
    let argument = argument.trim();
    argument.starts_with('&')
        || argument.starts_with("*const ")
        || argument.starts_with("*mut ")
        || ["Box<", "Rc<", "Arc<", "Cow<"].iter().any(|prefix| argument.starts_with(prefix))
}

fn is_numeric(argument: &str) -> bool {
    matches!(
        argument.trim(),
        "u8" | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "f32"
            | "f64"
    )
}

/// A few arguments, for a sentence.
fn sample(arguments: &[&str]) -> String {
    let shown: Vec<&str> = arguments.iter().take(3).copied().collect();
    if arguments.len() > shown.len() {
        format!("{}, …", shown.join(", "))
    } else {
        shown.join(", ")
    }
}
