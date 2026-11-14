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
fn computed(doc: &Document) -> String {
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
        "html.alt [data-deep]{border-color:red}",
        "html.alt [data-deep]{background:#eee}[data-deep]:hover{color:red}",
    ] {
        let old = page(style);
        let new = old.replace("<html>", "<html class=\"alt\">");
        assert_eq!(both(&old, &new), (false, false), "{style}");
    }
}
