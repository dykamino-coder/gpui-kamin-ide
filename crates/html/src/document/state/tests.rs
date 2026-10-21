//! Тесты владения: правила, метрики и арены значений принадлежат документу,
//! вложенный документ и второй документ в том же потоке их не трогают.

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
        .last("margin")
        .map(str::to_string)
}

#[test]
fn two_documents_keep_their_own_rules() {
    let a = Document::new(A, "");
    let b = Document::new(B, "");
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
    let a = Document::new(A, "");
    let inner = parse_embedded(B, "", (100.0, 100.0));
    assert!(a.values().doc.try_rule("--a").is_some());
    assert!(a.values().doc.try_rule("--b").is_none());
    assert_eq!(root_font_px(a.values()), 20.0);
    assert_eq!(margin(&a).as_deref(), Some("7px"));
    assert!(inner.state.try_rule("--b").is_some());
    assert_eq!(root_font_px(inner.state.values()), 10.0);
}

#[test]
fn parse_without_document_gets_its_own() {
    let a = Document::new(A, "");
    let parsed = crate::dom::parse(B, "");
    assert!(parsed.doc.try_rule("--b").is_some());
    assert!(a.values().doc.try_rule("--a").is_some());
    assert!(a.values().doc.try_rule("--b").is_none());
}

#[test]
fn medium_belongs_to_the_document() {
    const P: &str = "<!doctype html><style>@media print { html { font-size: 30px } }</style>";
    let print = Document::new_media(P, "", true);
    let screen = Document::new(P, "");
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
fn calc_handles_belong_to_their_document() {
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
#[cfg_attr(debug_assertions, should_panic(expected = "ручка арены без записи"))]
fn foreign_handle_is_a_link_error() {
    let a = super::DocState::new(Default::default());
    let b = super::DocState::new(Default::default());
    let h = calc_store(sum(1.0), a.values());
    assert_eq!(
        calc_get(h, b.values()).px,
        0.0,
        "explicit fallback in release"
    );
}

#[test]
#[cfg_attr(debug_assertions, should_panic(expected = "ручка арены без записи"))]
fn frame_region_is_reset_after_paint() {
    let a = super::DocState::new(Default::default());
    let h = calc_store(Sum::default(), a.frame_values());
    a.painted.set(true);
    a.begin_frame();
    calc_get(h, a.frame_values());
}
