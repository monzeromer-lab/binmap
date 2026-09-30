//! The inlining-aware flamegraph (`F3.3`, `U3.1`).
//!
//! A flamegraph is a prefix tree of stacks, where a node's width is how many
//! samples passed through it. The part `F3.3` asks for and most tools do not
//! give is **logical** call structure: with inline frames expanded, so the
//! function you wrote appears even though the machine never called it.
//!
//! Both views are real and the difference matters. The physical tree is what
//! the processor did; the logical tree is what the program says. A reader
//! looking for "why is my code slow" wants the logical one, and a reader
//! asking "why did inlining not help" wants the physical one — so the tree
//! carries the distinction rather than picking a side.

use crate::attribute::Attributed;
use crate::sample::Profile;
use binmap_crash::symbolize::Resolved;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One node in the flamegraph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub function: String,
    /// Samples that passed through here, including through children.
    pub samples: u64,
    /// Samples that stopped here — this node's own width in the graph.
    pub self_samples: u64,
    /// `F3.3`: inline frames visually distinct. An inlined function had no
    /// call overhead, so "stop calling it" is advice that cannot help.
    pub inlined: bool,
    /// Callees, widest first, so the eye lands on the hot path.
    pub children: Vec<Node>,
}

impl Node {
    pub fn share(&self, total: u64) -> f64 {
        if total == 0 {
            return 0.0;
        }
        self.samples as f64 / total as f64
    }

    /// Every node, depth-first, with its depth.
    pub fn walk(&self) -> Vec<(usize, &Node)> {
        let mut out = Vec::new();
        self.walk_into(0, &mut out);
        out
    }

    fn walk_into<'a>(&'a self, depth: usize, out: &mut Vec<(usize, &'a Node)>) {
        out.push((depth, self));
        for child in &self.children {
            child.walk_into(depth + 1, out);
        }
    }

    /// The widest path from here down — the hot path.
    ///
    /// What a reader wants first: not the hottest *function*, but the chain
    /// that leads to it.
    pub fn hot_path(&self) -> Vec<&str> {
        let mut path = vec![self.function.as_str()];
        let mut current = self;
        while let Some(widest) = current.children.first() {
            path.push(widest.function.as_str());
            current = widest;
        }
        path
    }

    /// How deep the tree goes.
    pub fn depth(&self) -> usize {
        1 + self.children.iter().map(Node::depth).max().unwrap_or(0)
    }
}

/// A whole flamegraph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flamegraph {
    pub root: Node,
    pub total_samples: u64,
    /// Whether inline frames were expanded.
    pub logical: bool,
}

impl Flamegraph {
    /// The one-line summary above the graph.
    pub fn describe(&self) -> String {
        format!(
            "{} samples, {} deep, {} call structure",
            self.total_samples,
            self.root.depth(),
            if self.logical { "logical" } else { "physical" }
        )
    }
}

/// Build a flamegraph.
///
/// `logical` expands inline frames, so a function the compiler inlined appears
/// as its own node. `false` gives the physical tree the processor actually
/// walked.
pub fn build<F>(profile: &Profile, logical: bool, mut resolve: F) -> Flamegraph
where
    F: FnMut(u64) -> Resolved,
{
    let mut cache: BTreeMap<u64, Resolved> = BTreeMap::new();
    for stack in profile.stacks.keys() {
        for address in &stack.addresses {
            cache.entry(*address).or_insert_with(|| resolve(*address));
        }
    }

    /// A node under construction, keyed by function name.
    #[derive(Default)]
    struct Building {
        samples: u64,
        self_samples: u64,
        inlined: bool,
        children: BTreeMap<String, Building>,
    }

    let mut root = Building::default();

    for (stack, count) in &profile.stacks {
        // A stack is sampled innermost first; a flamegraph reads outermost
        // first, so it is reversed. Getting this backwards produces a graph
        // that is upside down and looks perfectly plausible.
        let mut path: Vec<(String, bool)> = Vec::new();
        for address in stack.addresses.iter().rev() {
            let Some(resolved) = cache.get(address) else { continue };
            if resolved.is_empty() {
                continue;
            }
            if logical {
                // Outermost inline frame first, matching the outer-to-inner
                // direction the rest of the path runs in.
                for location in resolved.locations.iter().rev() {
                    if let Some(function) = &location.function {
                        path.push((function.clone(), location.inlined));
                    }
                }
            } else if let Some(function) =
                resolved.physical().and_then(|physical| physical.function.as_ref())
            {
                path.push((function.clone(), false));
            }
        }

        root.samples += count;
        let mut current = &mut root;
        for (function, inlined) in path {
            let child = current.children.entry(function).or_default();
            child.samples += count;
            child.inlined = inlined;
            current = child;
        }
        // The last node on the path is where the samples stopped.
        current.self_samples += count;
    }

    fn finish(function: String, building: Building) -> Node {
        let mut children: Vec<Node> =
            building.children.into_iter().map(|(name, child)| finish(name, child)).collect();
        // Widest first, so the eye lands on the hot path.
        children.sort_by(|left, right| {
            right.samples.cmp(&left.samples).then(left.function.cmp(&right.function))
        });
        Node {
            function,
            samples: building.samples,
            self_samples: building.self_samples,
            inlined: building.inlined,
            children,
        }
    }

    Flamegraph { total_samples: profile.total(), root: finish("all".to_string(), root), logical }
}

/// How a function's share changed between two profiles (`F3.4`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub function: String,
    pub before: u64,
    pub after: u64,
    /// `None` when the difference is inside sampling error — which is not the
    /// same as zero, and reporting it as zero would make noise look like
    /// stability.
    pub significant_delta: Option<i64>,
}

impl Change {
    pub fn delta(&self) -> i64 {
        self.after as i64 - self.before as i64
    }

    pub fn is_significant(&self) -> bool {
        self.significant_delta.is_some()
    }

    pub fn describe(&self) -> String {
        match self.significant_delta {
            Some(delta) if delta > 0 => {
                format!("{} got slower: {} → {} samples", self.function, self.before, self.after)
            }
            Some(_) => {
                format!("{} got faster: {} → {} samples", self.function, self.before, self.after)
            }
            None => format!(
                "{}: {} → {} samples, which is inside sampling error — not a change",
                self.function, self.before, self.after
            ),
        }
    }
}

/// Differential profiling (`F3.4`).
///
/// The discipline is the sweep's: a difference smaller than the measurement's
/// own error is not a difference. A profiler that ranks two functions three
/// samples apart out of four hundred is inventing an ordering, and a
/// regression hunt that starts from an invented ordering wastes a day.
pub fn compare(
    before: &Profile,
    after: &Profile,
    left: &Attributed,
    right: &Attributed,
) -> Vec<Change> {
    // Compared by *owner*, not by innermost location. "Which function
    // regressed" is a question about a function someone can edit, and time
    // spent in an inlined stdlib helper belongs to the function that inlined
    // it. Comparing by innermost location made a real injected regression in
    // `hotloop::transform` invisible, because every sample of it was
    // attributed to `<u64>::rotate_left`.
    let before_owners: std::collections::BTreeMap<String, u64> =
        left.by_owner().into_iter().collect();
    let after_owners: std::collections::BTreeMap<String, u64> =
        right.by_owner().into_iter().collect();

    let mut names: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    names.extend(before_owners.keys().map(String::as_str));
    names.extend(after_owners.keys().map(String::as_str));

    let find = |owners: &std::collections::BTreeMap<String, u64>, name: &str| {
        owners.get(name).copied().unwrap_or(0)
    };

    let mut changes: Vec<Change> = names
        .into_iter()
        .map(|name| {
            let first = find(&before_owners, name);
            let second = find(&after_owners, name);
            // Either profile's error could be the larger, and the comparison
            // is only as good as the worse of the two.
            let distinguishable =
                before.distinguishable(first, second) && after.distinguishable(first, second);
            Change {
                function: name.to_string(),
                before: first,
                after: second,
                significant_delta: distinguishable.then(|| second as i64 - first as i64),
            }
        })
        .collect();

    // Biggest regression first: that is what a regression hunt is for.
    changes.sort_by_key(|change| std::cmp::Reverse(change.significant_delta.unwrap_or(0)));
    changes
}
