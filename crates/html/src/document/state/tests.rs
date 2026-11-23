//! Rules and metrics belong to the document; expressions belong to their readers.
//! Nested documents and later documents on the same thread leave them intact.

use crate::document::{Document, parse_embedded};
use crate::style::css::page_decls_in;
use crate::style::values::value::{Sum, calc_get, calc_store, root_font_px};

const A: &str = "<!doctype html><style>html { font-size: 20px }
    @position-try --a { top: 1px }
    @page { margin: 7px }</style><p>a</p>";
const B: &str = "<!doctype html><style>html { font-size: 10px }
    @position-try --b { top: 2px }
    @page { margin: 9px }</style><p>b</p>";

fn margin(doc: &Document) -> Option<String> {
    page_decls_in(&doc.page_rules(), 0, "", false)
        .last("margin-top")
        .map(str::to_string)
}

#[test]
fn two_documents_keep_their_own_rules() {
    let a = Document::builder(A, "").build().unwrap();
    let b = Document::builder(B, "").build().unwrap();
    assert!(a.values().doc.try_rule("--a").is_some());
    assert!(a.values().doc.try_rule("--b").is_none());
    assert_eq!(root_font_px(a.values()), 20.0);
    assert!(b.values().doc.try_rule("--b").is_some());
    assert!(b.values().doc.try_rule("--a").is_none());
    assert_eq!(root_font_px(b.values()), 10.0);
    assert_eq!(margin(&a).as_deref(), Some("7px"));
    assert_eq!(margin(&b).as_deref(), Some("9px"));
}

#[test]
fn nested_document_does_not_touch_the_outer_one() {
    let a = Document::builder(A, "").build().unwrap();
    let inner = parse_embedded(B, "", (100.0, 100.0));
    assert!(a.values().doc.try_rule("--a").is_some());
    assert!(a.values().doc.try_rule("--b").is_none());
    assert_eq!(root_font_px(a.values()), 20.0);
    assert_eq!(margin(&a).as_deref(), Some("7px"));
    assert!(inner.state.values.try_rule("--b").is_some());
    assert_eq!(root_font_px(inner.state.values()), 10.0);
}

#[test]
fn parse_without_document_gets_its_own() {
    let a = Document::builder(A, "").build().unwrap();
    let parsed = crate::document::parsed::parse(B, "");
    assert!(parsed.doc.values.try_rule("--b").is_some());
    assert!(a.values().doc.try_rule("--a").is_some());
    assert!(a.values().doc.try_rule("--b").is_none());
}

#[test]
fn medium_belongs_to_the_document() {
    const P: &str = "<!doctype html><style>@media print { html { font-size: 30px } }</style>";
    let print = Document::builder(P, "").print(true).build().unwrap();
    let screen = Document::builder(P, "").build().unwrap();
    assert!(print.is_print() && !screen.is_print());
    assert_eq!(root_font_px(print.values()), 30.0);
    assert_eq!(root_font_px(screen.values()), 16.0);
}

fn sum(px: f32) -> Sum {
    Sum {
        px,
        pct: 0.5,
        has_percentage: true,
        ..Sum::default()
    }
}

#[test]
fn calc_expressions_keep_their_own_values() {
    let a = super::DocState::new(Default::default());
    let b = super::DocState::new(Default::default());
    let (ha, hb) = (
        calc_store(sum(1.0), a.values()),
        calc_store(sum(2.0), b.values()),
    );
    assert_ne!(ha, hb);
    assert_eq!(calc_get(ha, a.values()).px, 1.0);
    assert_eq!(calc_get(hb, b.values()).px, 2.0);
    // Ручка кадра читается в том же документе, из любой области записи.
    let hf = calc_store(sum(3.0), a.frame_values());
    assert_eq!(calc_get(hf, a.values()).px, 3.0);
}

#[test]
fn calc_expression_outlives_its_original_document() {
    let a = super::DocState::new(Default::default());
    let weak = std::rc::Rc::downgrade(&a.values);
    let h = calc_store(sum(1.0), a.values());
    drop(a);
    assert!(
        weak.upgrade().is_none(),
        "an expression must not own DocumentValues"
    );
    let b = super::DocState::new(Default::default());
    assert_eq!(calc_get(h, b.values()).px, 1.0);
}

#[test]
fn retained_expression_survives_frame_retirement() {
    let a = super::DocState::new(Default::default());
    let h = calc_store(sum(3.0), a.frame_values());
    a.values.mark_painted();
    a.begin_frame();
    assert_eq!(calc_get(h, a.frame_values()).px, 3.0);
}
