//! Проверки изоляции входов кэша и отделения групп при записи.
use super::*;
use crate::style::css::{Media, parse_decls_in};
use crate::values::DocumentValues;
use std::rc::Rc;

fn resolve(cache: &mut StyleSharing, inline: &DeclBlock, vcx: ValueCx<'_>) -> Computed {
    let map = CascadeMap::collect(&[], inline);
    cache.resolve(
        &[],
        inline,
        || &map,
        &VarMap::new(),
        CascadeCx::NONE,
        vcx,
        |_| {},
    )
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
    for width in 1..300 {
        let inline = parse_decls_in(&format!("width: {width}px"), vcx);
        let _ = resolve(&mut cache, &inline, vcx);
    }
    assert_eq!(cache.0.borrow().entries.len(), CAPACITY);
}

#[test]
fn a_cycle_of_ninety_one_inputs_hits_on_its_second_round() {
    let doc = DocumentValues::detached(Media::default());
    let vcx = doc.values();
    let mut cache = StyleSharing::default();
    let inputs: Vec<_> = (0..91)
        .map(|i| parse_decls_in(&format!("flex: 1 1 {}px", 60 + i), vcx))
        .collect();
    for _ in 0..2 {
        for inline in &inputs {
            let _ = resolve(&mut cache, inline, vcx);
        }
    }
    // A miss stores a new entry; the second round found every input.
    assert_eq!(cache.0.borrow().entries.len(), 91);
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

/// Поля, зависящие только от входов ключа, пишутся до сохранения: попадание
/// возвращает их готовыми и не отделяет общие группы записью.
#[test]
fn finish_runs_before_caching_and_hits_keep_shared_groups() {
    let doc = DocumentValues::detached(Media::default());
    let vcx = doc.values();
    let inline = parse_decls_in("width: 8px", vcx);
    let map = CascadeMap::collect(&[], &inline);
    let cache = StyleSharing::default();
    let calls = std::cell::Cell::new(0);
    let run = || {
        cache.resolve(
            &[],
            &inline,
            || &map,
            &VarMap::new(),
            CascadeCx::NONE,
            vcx,
            |style| {
                calls.set(calls.get() + 1);
                style.reset_cascade.embedding_dark_scheme = true;
            },
        )
    };
    let first = run();
    let second = run();
    assert_eq!(calls.get(), 1);
    assert!(second.reset_cascade.embedding_dark_scheme);
    assert!(first.shares_storage(&second));
}

/// Siblings with the same matched rules keep one storage for every group after
/// the whole element cascade, so materialization can reuse their computed style.
#[test]
fn equal_siblings_share_storage_after_element_cascade() {
    let parsed = crate::dom::parse(
        "<style>.c{display:inline-block;width:8px;background:#456}</style>\
         <p><i class=c></i><i class=c></i><i class=c></i></p>",
        "",
    );
    fn cells(nodes: &[crate::dom::Node], out: &mut Vec<Computed>) {
        for node in nodes {
            if let crate::dom::Node::Element(el) = node {
                if &*el.tag == "i" {
                    out.push(el.style.cascaded().clone());
                }
                cells(&el.children, out);
            }
        }
    }
    let mut found = Vec::new();
    cells(&parsed.nodes, &mut found);
    assert_eq!(found.len(), 3);
    assert!(found[0].shares_storage(&found[1]));
    assert!(found[1].shares_storage(&found[2]));
}

#[test]
fn gradients_are_pure_unless_a_nested_function_is_contextual() {
    for value in [
        "linear-gradient(90deg, #def, #fff)",
        "REPEATING-radial-gradient(circle, rgb(1 2 3) 2em, currentcolor 3em)",
        "conic-gradient(from 45deg, red, oklch(.5 .2 30))",
    ] {
        assert!(pure_value(value), "{value}");
    }
    for value in [
        "linear-gradient(var(--a), red)",
        "linear-gradient(red calc(10% + 1px), blue)",
        "radial-gradient(red, blue), url(a.png)",
        "-webkit-linear-gradient(red, blue)",
    ] {
        assert!(!pure_value(value), "{value}");
    }
}

/// A shared gradient computes exactly what the per-element cascade computes:
/// `currentcolor` and font-relative stops are resolved after the shared step.
#[test]
fn shared_gradients_compute_like_unshared_ones() {
    fn gradients(html: &str) -> Vec<(Computed, Rc<Computed>)> {
        fn cells(nodes: &[crate::dom::Node], out: &mut Vec<(Computed, Rc<Computed>)>) {
            for node in nodes {
                if let crate::dom::Node::Element(el) = node {
                    if &*el.tag == "i" {
                        out.push((el.style.cascaded().clone(), el.style.computed()));
                    }
                    cells(&el.children, out);
                }
            }
        }
        let mut out = Vec::new();
        cells(&crate::dom::parse(html, "").nodes, &mut out);
        out
    }
    let style = "<style>.g{background-image:linear-gradient(currentcolor, red 2em)}\
                 p{font-size:10px}</style>";
    let shared = gradients(&format!(
        "{style}<p style='color:#00f'><i class=g></i></p>\
         <p style='color:#0f0;font-size:20px'><i class=g></i></p>"
    ));
    let alone = gradients(&format!(
        "{style}<p style='color:#00f'><i class=g style='--a:1'></i></p>\
         <p style='color:#0f0;font-size:20px'><i class=g style='--a:2'></i></p>"
    ));
    assert_eq!((shared.len(), alone.len()), (2, 2));
    assert!(shared[0].0.shares_storage(&shared[1].0));
    for (s, a) in shared.iter().zip(&alone) {
        assert_eq!(s.1.reset_background.gradient, a.1.reset_background.gradient);
        assert_eq!(
            s.1.reset_background.gradient_current,
            a.1.reset_background.gradient_current
        );
    }
}

#[test]
fn hits_do_not_build_the_cascade_map() {
    let doc = DocumentValues::detached(Media::default());
    let vcx = doc.values();
    let inline = parse_decls_in("width: 8px; color: red", vcx);
    let map = CascadeMap::collect(&[], &inline);
    let cache = StyleSharing::default();
    let built = std::cell::Cell::new(0);
    let run = || {
        cache.resolve(
            &[],
            &inline,
            || {
                built.set(built.get() + 1);
                &map
            },
            &VarMap::new(),
            CascadeCx::NONE,
            vcx,
            |_| {},
        )
    };
    let first = run();
    let second = run();
    assert_eq!(built.get(), 1);
    assert!(first.reset_box_layout.ptr_eq(&second.reset_box_layout));
}
