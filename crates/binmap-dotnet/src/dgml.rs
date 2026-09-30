//! The ILC dependency graph (`TOOLING-DOTNET §2`).
//!
//! **This is the differentiator.** DWARF tells you what is in a binary; the
//! ILC graph tells you *why the compiler kept it* — the chain of references
//! from a root that stopped trimming from removing an item. `§2` is explicit
//! that this is "a first-class answer to 'why is this in my binary,' which the
//! native Rust backend cannot produce at all", and that it ships in the first
//! release rather than being a nice-to-have.
//!
//! The design's other instruction is the one worth repeating: **do not
//! reverse-engineer NativeAOT's symbol mangling.** The compiler already emits
//! structured accounting, so reading it is both easier and correct, and the
//! mangling is not a stable interface.
//!
//! DGML is XML with `<Nodes>` and `<Links>`. It is read streaming rather than
//! into a DOM because a graph for a real application has hundreds of thousands
//! of nodes, and holding all of them to answer one question about one of them
//! is the wrong shape.
//!
//! ⚠ `§2` warns that the format "has changed across .NET versions and is not
//! covered by a compatibility guarantee", so this reads defensively: an
//! unknown attribute is ignored, and a shape it does not recognise produces a
//! refusal naming the file rather than a graph with silent holes in it.

use binmap_core::error::{Error, Result};
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// One item the compiler kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    /// The managed name, in full. No demangling: `§2`'s point is that the
    /// compiler writes these out already.
    pub label: String,
}

impl Item {
    /// Which assembly this belongs to, where the label says so.
    ///
    /// ILC labels look like `Foo.Bar.Baz::Method()`, and the namespace root is
    /// the closest thing to an assembly the graph carries.
    pub fn namespace(&self) -> Option<&str> {
        let before_method = self.label.split("::").next()?;
        before_method.rsplit_once('.').map(|(namespace, _)| namespace)
    }

    /// Whether this looks like a method rather than a type.
    pub fn is_a_method(&self) -> bool {
        self.label.contains("::") || self.label.contains('(')
    }
}

/// Why one item kept another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reason {
    pub from: String,
    pub to: String,
    /// ILC's own words for the relationship, kept verbatim rather than mapped
    /// onto a vocabulary of ours: it is what the compiler said, and a reader
    /// who knows ILC will recognise it.
    pub because: String,
}

/// The whole graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    pub items: BTreeMap<String, Item>,
    pub reasons: Vec<Reason>,
}

impl Graph {
    /// Items nothing points at — the roots trimming starts from.
    pub fn roots(&self) -> Vec<&Item> {
        let reached: BTreeSet<&str> = self.reasons.iter().map(|r| r.to.as_str()).collect();
        self.items.values().filter(|item| !reached.contains(item.id.as_str())).collect()
    }

    /// Why an item is in the binary.
    ///
    /// The shortest chain from a root, because the shortest one is the most
    /// useful: a reader wants the reason, not every reason. Returns `None` for
    /// an item nothing reaches, which for a non-root means the graph
    /// contradicts itself and is worth knowing rather than papering over.
    pub fn why(&self, id: &str) -> Option<Vec<&Reason>> {
        if !self.items.contains_key(id) {
            return None;
        }
        let roots: BTreeSet<&str> = self.roots().iter().map(|item| item.id.as_str()).collect();
        if roots.contains(id) {
            // It is a root. Nothing kept it; it is why other things are here.
            return Some(Vec::new());
        }

        // Breadth-first from every root, so the first chain found is the
        // shortest.
        let mut seen: BTreeSet<&str> = roots.iter().copied().collect();
        let mut queue: VecDeque<(&str, Vec<&Reason>)> =
            roots.iter().map(|root| (*root, Vec::new())).collect();

        while let Some((current, path)) = queue.pop_front() {
            for reason in self.reasons.iter().filter(|reason| reason.from == current) {
                let mut extended = path.clone();
                extended.push(reason);
                if reason.to == id {
                    return Some(extended);
                }
                if seen.insert(reason.to.as_str()) {
                    queue.push_back((reason.to.as_str(), extended));
                }
            }
        }
        None
    }

    /// Why an item is here, as a sentence a reader can act on.
    pub fn explain(&self, id: &str) -> String {
        let Some(item) = self.items.get(id) else {
            return format!("`{id}` is not in this graph.");
        };
        match self.why(id) {
            Some(chain) if chain.is_empty() => format!(
                "{} is a root — it is why other things are here, not the other way round.",
                item.label
            ),
            Some(chain) => {
                let mut line = String::new();
                for (index, reason) in chain.iter().enumerate() {
                    if index == 0 {
                        line.push_str(self.label_of(&reason.from));
                    }
                    line.push_str(&format!(
                        "\n  ──{}──> {}",
                        reason.because,
                        self.label_of(&reason.to)
                    ));
                }
                line
            }
            None => format!(
                "{} is in the binary, but no chain from a root reaches it. That is the graph \
                 disagreeing with itself.",
                item.label
            ),
        }
    }

    fn label_of<'a>(&'a self, id: &'a str) -> &'a str {
        self.items.get(id).map(|item| item.label.as_str()).unwrap_or(id)
    }

    /// Everything one item is directly responsible for keeping.
    ///
    /// The other direction, and the one that answers "what would removing this
    /// save" — though only as an upper bound, because something else may keep
    /// the same item.
    pub fn kept_by(&self, id: &str) -> Vec<&Item> {
        self.reasons
            .iter()
            .filter(|reason| reason.from == id)
            .filter_map(|reason| self.items.get(&reason.to))
            .collect()
    }

    pub fn describe(&self) -> String {
        format!(
            "{} items and {} references, from {} roots",
            self.items.len(),
            self.reasons.len(),
            self.roots().len()
        )
    }
}

/// Read a `.dgml` file.
pub fn read(dgml: &[u8]) -> Result<Graph> {
    let mut reader = quick_xml::Reader::from_reader(dgml);
    reader.config_mut().trim_text(true);

    let mut items: BTreeMap<String, Item> = BTreeMap::new();
    let mut reasons = Vec::new();
    let mut buffer = Vec::new();
    let mut saw_a_known_element = false;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(element)) | Ok(Event::Empty(element)) => {
                let name = element.name();
                let name = String::from_utf8_lossy(name.as_ref()).to_string();

                // Attributes are read by name and unknown ones ignored: §2
                // warns the schema is not covered by a compatibility
                // guarantee, so a new attribute must not break the read.
                let attribute = |wanted: &str| -> Option<String> {
                    element.attributes().flatten().find_map(|attribute| {
                        (attribute.key.as_ref() == wanted.as_bytes())
                            .then(|| String::from_utf8_lossy(&attribute.value).to_string())
                    })
                };

                match name.as_str() {
                    "Node" => {
                        saw_a_known_element = true;
                        if let Some(id) = attribute("Id") {
                            let label = attribute("Label").unwrap_or_else(|| id.clone());
                            items.insert(id.clone(), Item { id, label });
                        }
                    }
                    "Link" => {
                        saw_a_known_element = true;
                        if let (Some(from), Some(to)) = (attribute("Source"), attribute("Target")) {
                            reasons.push(Reason {
                                from,
                                to,
                                // ILC puts the relationship in `Label`. Absent,
                                // the honest answer is that the graph recorded
                                // an edge and not a reason.
                                because: attribute("Label")
                                    .unwrap_or_else(|| "referenced".to_string()),
                            });
                        }
                    }
                    _ => {}
                }
            }
            Err(error) => {
                return Err(Error::Other(format!(
                    "this dependency graph could not be read: {error}. ILC writes it with \
                     `<IlcGenerateDgmlFile>true</IlcGenerateDgmlFile>`."
                )));
            }
            _ => {}
        }
        buffer.clear();
    }

    if !saw_a_known_element {
        return Err(Error::Other(
            "this is XML but not a DGML dependency graph: it has no <Node> or <Link> elements. \
             ILC writes one with `<IlcGenerateDgmlFile>true</IlcGenerateDgmlFile>`."
                .into(),
        ));
    }

    // An edge to an item the graph never declared is a real thing in a
    // truncated file. Kept rather than dropped, and named as unknown, because
    // dropping it would silently shorten every chain through it.
    let mut declared: Vec<Item> = Vec::new();
    for reason in &reasons {
        for id in [&reason.from, &reason.to] {
            if !items.contains_key(id) {
                declared.push(Item {
                    id: id.clone(),
                    label: format!("<{id}, which this graph does not name>"),
                });
            }
        }
    }
    for item in declared {
        items.entry(item.id.clone()).or_insert(item);
    }

    Ok(Graph { items, reasons })
}
