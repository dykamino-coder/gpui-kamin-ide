//! Тесты владения: правила и метрики принадлежат документу, вложенный
//! документ и второй документ в том же потоке их не трогают.

use crate::document::{Document, parse_embedded};
use crate::style::css::{page_decls_in, try_rule};
use crate::style::values::value::root_font_px;

const A: &str = "<!doctype html><style>html { font-size: 20px }
    @position-try --a { top: 1px }
    @page { margin: 7px }</style><p>a</p>";
const B: &str = "<!doctype html><style>html { font-size: 10px }
    @position-try --b { top: 2px }
    @page { margin: 9px }</style><p>b</p>";

fn margin(doc: &Document) -> Option<String> {
    page_decls_in(&doc.page_rules(), 0, "", false)
        .into_iter()
        .find(|(k, _)| k == "margin")
        .map(|(_, v)| v)
}

#[test]
fn two_documents_keep_their_own_rules() {
    let a = Document::new(A, "");
    let b = Document::new(B, "");
    {
        let _scope = a.enter();
        assert!(try_rule("--a").is_some());
        assert!(try_rule("--b").is_none());
        assert_eq!(root_font_px(), 20.0);
    }
    {
        let _scope = b.enter();
        assert!(try_rule("--b").is_some());
        assert!(try_rule("--a").is_none());
        assert_eq!(root_font_px(), 10.0);
    }
    assert_eq!(margin(&a).as_deref(), Some("7px"));
    assert_eq!(margin(&b).as_deref(), Some("9px"));
}

#[test]
fn nested_document_does_not_touch_the_outer_one() {
    let a = Document::new(A, "");
    let _scope = a.enter();
    let inner = parse_embedded(B, "", (100.0, 100.0));
    // Разбор рамки идёт в её состоянии; внешний документ — текущий, как был.
    assert!(try_rule("--a").is_some());
    assert!(try_rule("--b").is_none());
    assert_eq!(root_font_px(), 20.0);
    assert_eq!(margin(&a).as_deref(), Some("7px"));
    {
        let _inner = inner.state.enter();
        assert!(try_rule("--b").is_some());
        assert_eq!(root_font_px(), 10.0);
    }
    assert!(try_rule("--a").is_some());
}

#[test]
fn detached_parse_leaves_documents_alone() {
    let a = Document::new(A, "");
    let _ = crate::dom::parse(B, "");
    assert!(try_rule("--b").is_some(), "detached document of the thread");
    let _scope = a.enter();
    assert!(try_rule("--a").is_some());
    assert!(try_rule("--b").is_none());
}

#[test]
fn medium_belongs_to_the_document() {
    const P: &str = "<!doctype html><style>@media print { html { font-size: 30px } }</style>";
    let print = Document::new_media(P, "", true);
    let screen = Document::new(P, "");
    assert!(print.is_print() && !screen.is_print());
    {
        let _scope = print.enter();
        assert_eq!(root_font_px(), 30.0);
    }
    let _scope = screen.enter();
    assert_eq!(root_font_px(), 16.0);
}

fn calc_of(doc: &std::rc::Rc<super::DocState>, px: f32) -> crate::style::values::value::CalcId {
    use crate::style::values::value::{Sum, calc_store};
    let _scope = doc.enter();
    calc_store(Sum {
        px,
        pct: 0.5,
        has_percentage: true,
        ..Sum::default()
    })
}

#[test]
fn calc_handles_belong_to_their_document() {
    use crate::style::values::value::calc_get;
    let a = super::DocState::new(Default::default());
    let b = super::DocState::new(Default::default());
    let (ha, hb) = (calc_of(&a, 1.0), calc_of(&b, 2.0));
    assert_ne!(ha, hb);
    // Чужая ручка находит своего владельца и вне его области.
    let _scope = b.enter();
    assert_eq!(calc_get(ha).px, 1.0);
    assert_eq!(calc_get(hb).px, 2.0);
}

#[test]
#[cfg_attr(debug_assertions, should_panic(expected = "ручка арены без записи"))]
fn calc_arena_is_freed_with_its_document() {
    use crate::style::values::value::calc_get;
    let a = super::DocState::new(Default::default());
    let h = calc_of(&a, 1.0);
    drop(a);
    assert_eq!(calc_get(h).px, 0.0, "explicit fallback in release");
}

#[test]
#[cfg_attr(debug_assertions, should_panic(expected = "ручка арены без записи"))]
fn frame_region_is_reset_after_paint() {
    use crate::style::values::value::{Sum, calc_get, calc_store};
    let a = super::DocState::new(Default::default());
    let h = {
        let _scope = a.enter_frame();
        calc_store(Sum::default())
    };
    a.painted.set(true);
    a.begin_frame();
    let _scope = a.enter_frame();
    calc_get(h);
}
