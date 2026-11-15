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
