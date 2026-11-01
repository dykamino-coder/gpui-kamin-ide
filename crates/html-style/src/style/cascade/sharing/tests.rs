//! Проверки изоляции входов кэша и отделения групп при записи.
use super::*;
use crate::style::css::{Media, parse_decls_in};
use crate::values::DocumentValues;

fn resolve(cache: &mut StyleSharing, inline: &DeclBlock, vcx: ValueCx<'_>) -> Computed {
    let map = CascadeMap::collect(&[], inline);
    cache.resolve(&[], inline, &map, &VarMap::new(), CascadeCx::NONE, vcx)
}

#[test]
fn repeated_input_shares_groups_but_mutation_is_local() {
    let doc = DocumentValues::detached(Media::default());
    let vcx = doc.values();
    let inline = parse_decls_in("width: 12px; color: red", vcx);
    let mut cache = StyleSharing::default();
    let first = resolve(&mut cache, &inline, vcx);
    let mut second = resolve(&mut cache, &inline, vcx);
    assert!(first.reset_box_layout.ptr_eq(&second.reset_box_layout));
    second.reset_box_layout.width = Some(crate::style::values::value::Len::Px(99.0));
    assert!(!first.reset_box_layout.ptr_eq(&second.reset_box_layout));
    assert_eq!(
        first.reset_box_layout.width,
        Some(crate::style::values::value::Len::Px(12.0))
    );
    assert!(first.reset_border.ptr_eq(&second.reset_border));
}

#[test]
fn root_metrics_and_declarations_invalidate_cache() {
    let doc = DocumentValues::detached(Media::default());
    let vcx = doc.values();
    let inline = parse_decls_in("width: 2rem", vcx);
    let mut cache = StyleSharing::default();
    let _ = resolve(&mut cache, &inline, vcx);
    doc.root.borrow_mut().font_px = 24.0;
    let _ = resolve(&mut cache, &inline, vcx);
    assert_eq!(cache.0.borrow().entries.len(), 2);
    let changed = parse_decls_in("width: 3rem", vcx);
    let _ = resolve(&mut cache, &changed, vcx);
    assert_eq!(cache.0.borrow().entries.len(), 3);
}

#[test]
fn context_functions_escapes_and_root_are_not_cached() {
    let doc = DocumentValues::detached(Media::default());
    let vcx = doc.values();
    for text in [
        "width: attr(data-width px)",
        "width: calc(1px + 2px)",
        "color: var(--color)",
        "width: sibling-index()",
        r"width: c\61lc(1px)",
    ] {
        let inline = parse_decls_in(text, vcx);
        assert!(
            cacheable(&[], &inline, CascadeCx::NONE, vcx).is_none(),
            "{text}"
        );
    }
    let inline = parse_decls_in("color: red", vcx);
    let cx = CascadeCx {
        root: true,
        ..CascadeCx::NONE
    };
    assert!(cacheable(&[], &inline, cx, vcx).is_none());
}

#[test]
fn cache_is_bounded() {
    let doc = DocumentValues::detached(Media::default());
    let vcx = doc.values();
    let mut cache = StyleSharing::default();
    for width in 1..80 {
        let inline = parse_decls_in(&format!("width: {width}px"), vcx);
        let _ = resolve(&mut cache, &inline, vcx);
    }
    assert_eq!(cache.0.borrow().entries.len(), 32);
}

#[test]
fn only_context_free_color_functions_are_admitted() {
    for value in ["rgb(1, 2, 3)", "HSL(30 10% 80%)", "oklch(.5 .2 30)"] {
        assert!(pure_value(value), "{value}");
    }
    for value in [
        "rgb(from var(--base) r g b)",
        "rgb(attr(data-r number) 0 0)",
        "rgb(0 0 calc(1 + 2))",
        r"r\67 b(0 0 0)",
    ] {
        assert!(!pure_value(value), "{value}");
    }
}
