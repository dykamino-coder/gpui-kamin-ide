//! Серии правок страницы с `:has()`: метки `:has()` пробы держатся между
//! правками на месте (`RestyleCache::has`), и каждая ревизия равна свежей
//! сборке, в том числе после правки имени из аргумента `:has()` и после
//! правки текста.

use super::{state, tree};
use crate::document::Document;
use std::rc::Rc;

const PAGE: &str = "<!doctype html><html><head><style>\
    .box:has(.hot){background:#eee}.box:has(p:empty){color:#123}\
    [data-edit]{background:#ddd}</style></head><body>\
    <div class=box><p class=@1>a</p><p>b</p></div><div class=box><p>c</p>\
    <span@2>d</span></div></body></html>";

fn page(hot: bool, edit: bool, text: &str) -> String {
    PAGE.replace("@1", if hot { "hot" } else { "cold" })
        .replace("@2", if edit { " data-edit" } else { "" })
        .replace(">b<", &format!(">{text}<"))
}

/// Провести документ по состояниям; число ревизий, принятых на месте.
fn run(steps: &[(bool, bool, &str)]) -> usize {
    let (h, e, t) = steps[0];
    let mut doc = Document::builder(&page(h, e, t), "").build().unwrap();
    let mut kept = 0;
    for &(h, e, t) in &steps[1..] {
        let markup = page(h, e, t);
        let values = doc.state.values.clone();
        assert!(doc.update(&markup, "").unwrap().changed());
        kept += usize::from(Rc::ptr_eq(&values, &doc.state.values));
        let fresh = Document::builder(&markup, "").build().unwrap();
        assert_eq!(tree(&doc), tree(&fresh), "{h} {e} {t:?}");
        assert_eq!(state(&doc), state(&fresh), "{h} {e} {t:?}");
    }
    kept
}

#[test]
fn edits_outside_has_arguments_keep_the_marks_and_match_fresh_builds() {
    let on = (true, true, "b");
    let off = (true, false, "b");
    assert_eq!(run(&[off, on, off, on, off]), 4);
}

#[test]
fn an_edit_of_a_has_argument_or_a_text_between_edits_matches_fresh_builds() {
    run(&[
        (false, false, "b"),
        (false, true, "b"),
        (true, true, "b"),
        (true, false, "b"),
        (true, false, "bb"),
        (true, true, "bb"),
        (false, true, "bb"),
        (false, false, "bb"),
    ]);
}

/// Провести документ по разметкам `pages`; каждая ревизия равна свежей
/// сборке. Число ревизий, принятых на месте.
fn series(pages: &[String]) -> usize {
    let mut doc = Document::builder(&pages[0], "").build().unwrap();
    let mut kept = 0;
    for markup in &pages[1..] {
        let values = doc.state.values.clone();
        assert!(doc.update(markup, "").unwrap().changed());
        kept += usize::from(Rc::ptr_eq(&values, &doc.state.values));
        let fresh = Document::builder(markup, "").build().unwrap();
        assert_eq!(tree(&doc), tree(&fresh), "{markup}");
        assert_eq!(state(&doc), state(&fresh), "{markup}");
    }
    kept
}

const ARGS: &str = "<!doctype html><html><head><style>\
    .box:has([data-arg]){background:#eee}.box:has(.cls){color:#123}\
    .box:has(> p + p){border:1px solid #000}.box:has(:checked){outline:1px solid #f00}\
    [data-edit]{background:#ddd}</style></head><body>\
    <div class=box><p@1>a</p><p@2>b</p>@3<input type=checkbox@4></div>\
    <div class=box><span@5>c</span></div></body></html>";

/// `ARGS` с атрибутами мест `@N` и вставкой `@3`.
fn args(at: &[(usize, &str)]) -> String {
    let mut out = ARGS.to_string();
    for n in 1..=5 {
        let add = at.iter().filter(|(p, _)| *p == n).map(|(_, a)| *a);
        out = out.replace(&format!("@{n}"), &add.collect::<String>());
    }
    out
}

#[test]
fn edits_of_has_arguments_between_kept_edits_match_fresh_builds() {
    let edit = (5, " data-edit");
    let pages = [
        args(&[]),
        args(&[edit]),
        // An attribute and a class read inside `:has()`.
        args(&[edit, (1, " data-arg")]),
        args(&[(1, " data-arg")]),
        args(&[(1, " data-arg"), (2, " class=cls")]),
        args(&[(1, " data-arg"), (2, " class=cls"), edit]),
        // A child inserted and removed under a `:has()` anchor.
        args(&[(3, "<p>x</p>"), edit]),
        args(&[(3, "<p>x</p>")]),
        args(&[]),
        // A state read inside `:has()`.
        args(&[(4, " checked")]),
        args(&[(4, " checked"), edit]),
        args(&[edit]),
    ];
    let kept = series(&pages);
    assert!(kept >= 3, "{kept} in place");
}
