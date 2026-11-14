//! Серии правок одного документа (`revision::repaint` с пробой, которая
//! держит правила и контексты входа между правками, `walk::Chains`): каждая
//! ревизия серии равна свежей сборке той же разметки, а правки, которые
//! меняют только краску, принимаются на месте и после чужих правок.

use super::repaint_background::computed;
use super::{state, tree};
use crate::document::Document;
use std::rc::Rc;

/// Страница с местами атрибутов `@N` в открывающих тегах.
const PAGE: &str = "<!doctype html><html@0><head><style>\
    html,body{margin:0;color:#000;background:#fff}\
    .pick .t{background:#eee}.pick [data-e]{background:#ddd}\
    [data-a] + [data-b]{background:#ccc}[data-a] + div [data-c]{background:#bbb}\
    .grp > p{color:#123}html.alt [data-deep]{background:#aaa}.zed{color:#456}\
    li:nth-child(2n){color:#321}.pick li{background:#888}\
    .wide{margin-left:1px}</style></head><body>\
    <div id=a@1><p class=t>1</p><p data-deep@2>2 <span@3>3</span></p></div>\
    <div@4>s</div><div@5><div@6><p@7>4</p><span@8>5</span></div></div>\
    <ul@9><li@10>6</li><li@11>7</li><li@12>8</li></ul></body></html>";

/// Правки: место и атрибут (`.имя` — класс, иначе признак `data-*`).
const TOGGLES: &[(usize, &str)] = &[
    (0, ".alt"),
    (1, ".pick"),
    (1, ".grp"),
    (2, "data-e"),
    (3, "data-e"),
    (3, ".zed"),
    (4, "data-a"),
    (5, "data-b"),
    (5, ".pick"),
    (6, ".grp"),
    (7, "data-c"),
    (7, "data-deep"),
    (8, "data-e"),
    (9, ".pick"),
    (10, "data-a"),
    (11, "data-b"),
    (11, ".pick"),
    (12, ".zed"),
    (6, ".wide"),
];

const PLACES: usize = 13;

fn page(on: &[bool]) -> String {
    let mut out = PAGE.to_string();
    for place in (0..PLACES).rev() {
        let active = |class: bool| {
            let at = TOGGLES.iter().zip(on);
            at.filter(move |((p, name), on)| *p == place && **on && name.starts_with('.') == class)
                .map(|((_, name), _)| name.trim_start_matches('.'))
        };
        let classes: Vec<&str> = active(true).collect();
        let mut attrs = String::new();
        if !classes.is_empty() {
            attrs.push_str(&format!(" class=\"{}\"", classes.join(" ")));
        }
        for name in active(false) {
            attrs.push_str(&format!(" {name}=\"1\""));
        }
        out = out.replace(&format!("@{place}>"), &format!("{attrs}>"));
    }
    out
}

/// Провести документ по состояниям `steps`; число ревизий, принятых на месте.
fn run(steps: &[Vec<bool>]) -> usize {
    let mut doc = Document::builder(&page(&steps[0]), "").build().unwrap();
    computed(&doc);
    let mut kept = 0;
    for on in &steps[1..] {
        let markup = page(on);
        let values = doc.state.values.clone();
        assert!(doc.update(&markup, "").unwrap().changed());
        kept += usize::from(Rc::ptr_eq(&values, &doc.state.values));
        let fresh = Document::builder(&markup, "").build().unwrap();
        assert_eq!(computed(&doc), computed(&fresh), "{on:?}");
        assert_eq!(tree(&doc), tree(&fresh), "{on:?}");
        assert_eq!(state(&doc), state(&fresh), "{on:?}");
    }
    kept
}

fn states(flips: &[usize]) -> Vec<Vec<bool>> {
    let mut on = vec![false; TOGGLES.len()];
    let mut out = vec![on.clone()];
    for &flip in flips {
        on[flip] = !on[flip];
        out.push(on.clone());
    }
    out
}

#[test]
fn a_root_class_toggled_back_and_forth_stays_in_place() {
    assert_eq!(run(&states(&[0, 0, 0, 0, 0])), 5);
}

#[test]
fn a_later_edit_matches_through_the_attributes_an_earlier_one_left() {
    // `.pick` on the ancestor, then `data-e` on two elements below it: the
    // kept path carries the class the first edit left.
    assert_eq!(run(&states(&[1, 3, 4, 1, 3, 4])), 6);
    // A list class reaches the items below it and leaves the item beside.
    assert_eq!(run(&states(&[13, 16, 13, 16, 13])), 5);
    // A name read in a sibling compound is rebuilt (`data-a`), so the census
    // kept from before answers for it when the neighbour is edited in place.
    assert_eq!(run(&states(&[14, 15, 14, 15])), 2);
    assert_eq!(run(&states(&[10, 6, 10, 10, 6])), 3);
}

#[test]
fn a_rebuild_between_edits_starts_the_probe_over() {
    let last = TOGGLES.len() - 1;
    assert_eq!(run(&states(&[0, 10, last, 10, 0, last, 0, 10])), 6);
}

#[test]
fn random_series_match_fresh_builds() {
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..4 {
        let flips: Vec<usize> = (0..40)
            .map(|_| (next() % TOGGLES.len() as u64) as usize)
            .collect();
        let kept = run(&states(&flips));
        assert!(kept >= 20, "{kept} of 40 in place: {flips:?}");
    }
}
