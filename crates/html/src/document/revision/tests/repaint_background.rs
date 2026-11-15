//! Смены `class` и `data-*`, чей перерасчёт стиля меняет собственный фон
//! коробки (`revision::repaint`): ревизия без сборки равна свежей сборке и
//! деревом коробок, и вычисленными стилями после материализации под уже
//! материализованным прежним деревом; фон холста, значения раскладки и копии
//! фона вне каскада собираются заново.

use super::update;
use crate::document::Document;
use crate::dom::Node;
use crate::style::cascade::materialize::materialize;
use std::rc::Rc;

fn page(style: &str) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;color:#000;background:#fff}}{style}</style>\
         </head><body><h1>t</h1><div class=box><p data-deep>a <span>b</span></p><p class=own>c</p>\
         </div><ul><li>x</li></ul></body></html>"
    )
}

/// Обе стороны правки.
fn both(old: &str, new: &str) -> (bool, bool) {
    (update(old, new, ""), update(new, old, ""))
}

/// Вычисленные стили элементов дерева после материализации.
pub(super) fn computed(doc: &Document) -> String {
    fn walk(nodes: &[Node], out: &mut String) {
        for node in nodes {
            if let Node::Element(e) = node {
                out.push_str(&format!("{} {:?}\n", e.tag, e.style.computed()));
                walk(&e.children, out);
            }
        }
    }
    let root = Rc::new(doc.root_style().clone());
    materialize(doc.nodes(), &root, doc.state.values());
    let mut out = String::new();
    walk(doc.nodes(), &mut out);
    out
}

/// Ревизия `old` → `new` принята на месте и после кадра прежней ревизии даёт
/// те же вычисленные стили, что свежая сборка `new`.
fn materialized(old: &str, new: &str) {
    let mut doc = Document::builder(old, "").build().unwrap();
    let values = doc.state.values.clone();
    let before = computed(&doc);
    assert!(doc.update(new, "").unwrap().changed());
    assert!(Rc::ptr_eq(&values, &doc.state.values), "{old} -> {new}");
    let fresh = Document::builder(new, "").build().unwrap();
    let after = computed(&doc);
    assert_eq!(after, computed(&fresh), "{old} -> {new}");
    assert_ne!(before, after, "{old} -> {new}");
}

#[test]
fn a_root_class_that_paints_the_background_of_a_deep_element_is_restyled_in_place() {
    let old = page("html.alt [data-deep]{background:#eee}");
    let new = old.replace("<html>", "<html class=\"alt\">");
    assert_eq!(both(&old, &new), (true, true));
    materialized(&old, &new);
    materialized(&new, &old);
}

#[test]
fn a_data_attribute_that_paints_the_background_of_its_element_is_restyled_in_place() {
    let old = page("[data-edit]{background:#eee}[data-edit=\"2\"]{background:#ddd}");
    let one = old.replace("<span>", "<span data-edit=\"1\">");
    let two = old.replace("<span>", "<span data-edit=\"2\">");
    assert_eq!(both(&old, &one), (true, true));
    assert_eq!(both(&one, &two), (true, true));
    materialized(&old, &one);
    materialized(&one, &two);
    // A block, and a descendant found by its own attribute.
    let old = page("[data-edit] [data-deep]{background:#eee}");
    let new = old.replace("class=box", "class=box data-edit");
    assert_eq!(both(&old, &new), (true, true));
    materialized(&old, &new);
}

#[test]
fn a_background_and_a_colour_together_are_restyled_in_place() {
    let old = page(".box.hot{color:red;background:#ff0}.own{color:#00f}");
    let new = old.replace("class=box", "class=\"box hot\"");
    assert_eq!(both(&old, &new), (true, true));
    materialized(&old, &new);
    materialized(&new, &old);
}

#[test]
fn a_child_that_inherits_the_background_follows_by_materialization() {
    let old = page(".box.hot{background:#ff0}.own{background-color:inherit}");
    let new = old.replace("class=box", "class=\"box hot\"");
    assert_eq!(both(&old, &new), (true, true));
    materialized(&old, &new);
    materialized(&new, &old);
}

#[test]
fn the_canvas_background_layout_values_and_hover_layers_rebuild() {
    for style in [
        "html.alt body{background:#eee}",
        "html.alt{background:#eee}",
        "html.alt [data-deep]{background:#eee;padding:1px}",
        "html.alt [data-deep]{background:url(a.png)}",
        "html.alt [data-deep]{background:#eee}[data-deep]:hover{color:red}",
    ] {
        let old = page(style);
        let new = old.replace("<html>", "<html class=\"alt\">");
        assert_eq!(both(&old, &new), (false, false), "{style}");
    }
}

/// A page whose `html` and `body` have no box (`document::unwrap_document`):
/// their values live in the cascades of the top-level boxes.
fn unwrapped(style: &str) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;font:13px serif;color:#000}}{style}\
         </style></head><body><h1>t</h1><div class=box><p data-deep>a <span>b</span></p>\
         <p class=own>c</p></div><ul><li>x</li></ul></body></html>"
    )
}

fn folded(html: &str) -> bool {
    let doc = Document::builder(html, "").build().unwrap();
    !doc.nodes()
        .iter()
        .any(|n| matches!(n, Node::Element(e) if matches!(e.tag.as_str(), "html" | "body")))
}

#[test]
fn a_descendant_under_unwrapped_wrappers_is_restyled_in_place() {
    let old = unwrapped(".hot{color:red;background:#ff0}");
    assert!(folded(&old));
    let new = old.replace("<span>", "<span class=hot>");
    assert_eq!(both(&old, &new), (true, true));
    materialized(&old, &new);
    materialized(&new, &old);
}

/// The box of a child of the unwrapped `body` folds the wrapper's values
/// into its cascade; the probe folds them the same way.
#[test]
fn a_direct_child_of_an_unwrapped_body_is_restyled_in_place() {
    let old = unwrapped(".box.hot{color:red;background:#ff0}.own{color:#00f}");
    let new = old.replace("class=box", "class=\"box hot\"");
    assert_eq!(both(&old, &new), (true, true));
    materialized(&old, &new);
    materialized(&new, &old);
}

/// An unwrapped wrapper has no box: its restyle reaches every top-level box
/// and the text style of the page, and the document is rebuilt.
#[test]
fn a_restyled_unwrapped_wrapper_rebuilds_equal_to_a_fresh_build() {
    for (style, tag) in [
        ("body.alt{color:red}", "<body>"),
        (".alt{color:red}", "<html>"),
    ] {
        let old = unwrapped(style);
        let new = old.replace(tag, &tag.replace('>', " class=\"alt\">"));
        assert!(folded(&old) && folded(&new), "{style}");
        assert_eq!(both(&old, &new), (false, false), "{style}");
    }
}

/// Sheets parsed by `edit`.
#[cfg(feature = "work-counters")]
fn sheets_parsed(edit: impl FnOnce()) -> u64 {
    use crate::work::{Counter, Session};
    let session = Session::begin();
    edit();
    let work = session.finish();
    work.0
        .iter()
        .map(|phase| phase[Counter::SheetsParsed as usize])
        .sum()
}

/// The probe of a document is the rules of its build, kept between its
/// edits: neither the first edit after a build nor a steady one parses a
/// sheet.
#[cfg(feature = "work-counters")]
#[test]
fn a_steady_edit_parses_no_sheet() {
    let old = page("html.alt [data-deep]{background:#eee}");
    let new = old.replace("<html>", "<html class=\"alt\">");
    let mut doc = Document::builder(&old, "").build().unwrap();
    let parsed = sheets_parsed(|| {
        assert!(doc.update(&new, "").unwrap().changed());
        assert!(doc.update(&old, "").unwrap().changed());
        assert!(doc.update(&new, "").unwrap().changed());
    });
    assert_eq!(parsed, 0);
}

/// An edit that reaches layout rebuilds: it parses the sheets of one build
/// (agent, theme and the page's `<style>`), not a probe's copy as well, and
/// the next paint-only edit parses none.
#[cfg(feature = "work-counters")]
#[test]
fn a_rebuilding_edit_parses_the_sheets_once() {
    let old = page("html.alt [data-deep]{padding:1px}.box.hot{background:#ff0}");
    let new = old.replace("<html>", "<html class=\"alt\">");
    let mut doc = Document::builder(&old, "").build().unwrap();
    let parsed = sheets_parsed(|| assert!(doc.update(&new, "").unwrap().changed()));
    assert_eq!(parsed, 3);
    let hot = new.replace("class=box", "class=\"box hot\"");
    let parsed = sheets_parsed(|| assert!(doc.update(&hot, "").unwrap().changed()));
    assert_eq!(parsed, 0);
}
