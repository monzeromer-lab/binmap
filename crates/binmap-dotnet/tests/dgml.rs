//! The ILC dependency graph (`TOOLING-DOTNET §2`).
//!
//! The DGML here is hand-written to the documented shape, and that is stated
//! rather than hidden: there is no .NET SDK on this machine, so it has not
//! been read against real ILC output. `§2` warns the schema "has changed
//! across .NET versions and is not covered by a compatibility guarantee",
//! which is why the reader ignores unknown attributes and refuses a shape it
//! does not recognise instead of producing a graph with holes in it.

use binmap_dotnet::dgml::read;

/// A small graph in DGML's documented shape.
///
/// `Main` is the root; it calls `Serialize`, which reflects over `Widget`,
/// which is why `Widget` survived trimming. That chain is the question the
/// whole feature exists to answer.
const GRAPH: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<DirectedGraph xmlns="http://schemas.microsoft.com/vs/2009/dgml">
  <Nodes>
    <Node Id="1" Label="App.Program::Main()" />
    <Node Id="2" Label="App.Serializer::Serialize(System.Object)" />
    <Node Id="3" Label="App.Models.Widget" />
    <Node Id="4" Label="App.Models.Widget::.ctor()" />
    <Node Id="5" Label="App.Unused::Never()" />
  </Nodes>
  <Links>
    <Link Source="1" Target="2" Label="Direct Call" />
    <Link Source="2" Target="3" Label="Reflection" />
    <Link Source="3" Target="4" Label="Constructed Type" />
  </Links>
</DirectedGraph>"#;

#[test]
fn nodes_and_links_are_read_with_their_managed_names() {
    // §2: "full managed names, no demangling required" — the whole reason for
    // reading the compiler's accounting rather than the symbol table.
    let graph = read(GRAPH.as_bytes()).expect("a documented DGML shape");

    assert_eq!(graph.items.len(), 5);
    assert_eq!(graph.reasons.len(), 3);
    assert_eq!(graph.items["3"].label, "App.Models.Widget");
    assert!(graph.describe().contains("5 items"), "{}", graph.describe());
}

#[test]
fn why_an_item_is_in_the_binary_is_the_chain_from_a_root() {
    // The differentiator. DWARF says what is in a binary and cannot say why.
    let graph = read(GRAPH.as_bytes()).unwrap();

    let chain = graph.why("3").expect("Widget is reachable");
    assert_eq!(chain.len(), 2, "Main → Serialize → Widget");
    assert_eq!(chain[0].from, "1");
    assert_eq!(chain[1].because, "Reflection", "ILC's own word for it");

    let explained = graph.explain("3");
    assert!(explained.contains("App.Program::Main"), "{explained}");
    assert!(explained.contains("Reflection"), "{explained}");
    assert!(explained.contains("App.Models.Widget"), "{explained}");
}

#[test]
fn a_root_is_why_other_things_are_here_rather_than_the_other_way_round() {
    let graph = read(GRAPH.as_bytes()).unwrap();

    let roots = graph.roots();
    let root_ids: Vec<&str> = roots.iter().map(|item| item.id.as_str()).collect();
    assert!(root_ids.contains(&"1"), "Main is a root: {root_ids:?}");
    assert!(root_ids.contains(&"5"), "so is an item nothing references");

    assert!(graph.why("1").expect("a root").is_empty());
    assert!(graph.explain("1").contains("is a root"), "{}", graph.explain("1"));
}

#[test]
fn the_shortest_chain_is_the_one_reported() {
    // A reader wants the reason, not every reason.
    let graph = read(
        r#"<DirectedGraph>
             <Nodes>
               <Node Id="root" Label="Root" />
               <Node Id="mid" Label="Middle" />
               <Node Id="leaf" Label="Leaf" />
             </Nodes>
             <Links>
               <Link Source="root" Target="leaf" Label="Direct Call" />
               <Link Source="root" Target="mid" Label="Direct Call" />
               <Link Source="mid" Target="leaf" Label="Direct Call" />
             </Links>
           </DirectedGraph>"#
            .as_bytes(),
    )
    .unwrap();

    assert_eq!(graph.why("leaf").expect("reachable").len(), 1, "the direct route");
}

#[test]
fn a_cycle_does_not_hang_the_explanation() {
    let graph = read(
        r#"<DirectedGraph>
             <Nodes>
               <Node Id="a" Label="A" /><Node Id="b" Label="B" /><Node Id="c" Label="C" />
             </Nodes>
             <Links>
               <Link Source="a" Target="b" /><Link Source="b" Target="c" />
               <Link Source="c" Target="b" />
             </Links>
           </DirectedGraph>"#
            .as_bytes(),
    )
    .unwrap();

    assert!(graph.why("c").is_some());
    assert!(graph.why("nothing-like-this").is_none());
}

#[test]
fn what_an_item_keeps_is_the_other_direction() {
    // "What would removing this save" — an upper bound, because something
    // else may keep the same item.
    let graph = read(GRAPH.as_bytes()).unwrap();
    let kept = graph.kept_by("2");
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].label, "App.Models.Widget");
}

#[test]
fn an_edge_to_an_item_the_graph_never_declared_is_named_rather_than_dropped() {
    // Real in a truncated file, and dropping it would silently shorten every
    // chain that runs through it.
    let graph = read(
        r#"<DirectedGraph>
             <Nodes><Node Id="a" Label="A" /></Nodes>
             <Links><Link Source="a" Target="ghost" Label="Direct Call" /></Links>
           </DirectedGraph>"#
            .as_bytes(),
    )
    .unwrap();

    assert_eq!(graph.items.len(), 2);
    assert!(graph.items["ghost"].label.contains("does not name"), "{:?}", graph.items["ghost"]);
}

#[test]
fn an_unknown_attribute_does_not_break_the_read() {
    // §2 warns the schema is not covered by a compatibility guarantee, so a
    // new attribute must not be fatal.
    let graph = read(
        r#"<DirectedGraph>
             <Nodes><Node Id="a" Label="A" SomethingNew="42" Category="Type" /></Nodes>
             <Links><Link Source="a" Target="a" NewThing="x" /></Links>
           </DirectedGraph>"#
            .as_bytes(),
    )
    .expect("unknown attributes are ignored");
    assert_eq!(graph.items["a"].label, "A");
}

#[test]
fn xml_that_is_not_a_dependency_graph_says_how_to_produce_one() {
    let error =
        read(b"<?xml version=\"1.0\"?><project><thing/></project>").expect_err("not a DGML graph");
    assert!(error.to_string().contains("not a DGML"), "{error}");
    assert!(error.to_string().contains("IlcGenerateDgmlFile"), "{error}");
}

#[test]
fn something_that_is_not_xml_at_all_is_refused() {
    assert!(read(b"\x00\x01 binary nonsense <<<").is_err());
}

#[test]
fn a_node_with_no_label_falls_back_to_its_identifier() {
    // Better than an empty row: the id is at least something to search for.
    let graph =
        read(r#"<DirectedGraph><Nodes><Node Id="42" /></Nodes></DirectedGraph>"#.as_bytes())
            .unwrap();
    assert_eq!(graph.items["42"].label, "42");
}

#[test]
fn a_link_with_no_label_says_referenced_rather_than_inventing_a_reason() {
    let graph = read(
        r#"<DirectedGraph>
             <Nodes><Node Id="a" /><Node Id="b" /></Nodes>
             <Links><Link Source="a" Target="b" /></Links>
           </DirectedGraph>"#
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(graph.reasons[0].because, "referenced");
}

#[test]
fn a_managed_name_yields_its_namespace_and_whether_it_is_a_method() {
    let graph = read(GRAPH.as_bytes()).unwrap();
    assert_eq!(graph.items["1"].namespace(), Some("App"));
    assert!(graph.items["1"].is_a_method());
    assert!(!graph.items["3"].is_a_method(), "Widget is a type");
}
