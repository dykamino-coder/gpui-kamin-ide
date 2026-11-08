//! Повреждение пакета (`commit::Plan`): пакет без единой правленой коробки
//! сохраняет дерево, сохраняемый кадр и состояние ревизии; пакет с правками
//! коробок копирует только пути. Обе ревизии равны свежей сборке разметки.

use super::super::Mutation;
use super::{PAGE, state, tree};
use crate::document::Document;
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

fn element(node: &Handle, local: &str) -> Option<Handle> {
    if matches!(&node.data, NodeData::Element { name, .. } if &*name.local == local) {
        return Some(node.clone());
    }
    node.children
        .borrow()
        .iter()
        .find_map(|c| element(c, local))
}

fn attr(element: &Handle, name: &str, value: &str) -> Mutation {
    Mutation::Attribute {
        element: element.clone(),
        name: html5ever::QualName::new(None, html5ever::ns!(), name.into()),
        value: Some(value.into()),
    }
}

/// Применить запись к `old` (элемент `local`, атрибут `name`): равна ли
/// ревизия свежей сборке `new`, и сохранены ли кадр и состояние (`Some`:
/// и дерево тоже — ни одна коробка не правлена).
fn kept(old: &str, local: &str, name: &str, value: &str, new: &str) -> Option<bool> {
    let mut doc = Document::builder(old, "").build().unwrap();
    let (nodes, slot, state_rc) = (doc.nodes.clone(), doc.retained.clone(), doc.state.clone());
    let target = element(&doc.source_dom(), local).expect("element");
    assert!(
        doc.apply(&[attr(&target, name, value)], "")
            .unwrap()
            .changed()
    );
    let fresh = Document::builder(new, "").build().unwrap();
    assert_eq!(tree(&doc), tree(&fresh), "{old} -> {new}");
    assert_eq!(state(&doc), state(&fresh), "{old} -> {new}");
    let frame = Rc::ptr_eq(&slot, &doc.retained);
    assert_eq!(
        frame,
        Rc::ptr_eq(&state_rc, &doc.state),
        "frame and state go together"
    );
    frame.then(|| Rc::ptr_eq(&nodes, &doc.nodes))
}

#[test]
fn records_no_frame_reads_keep_the_frame_and_the_state() {
    let classed = PAGE.replace("<html>", "<html class=\"alt\">");
    assert_eq!(kept(PAGE, "html", "class", "alt", &classed), Some(true));
    // Коробка `p` получает новый атрибут, но кадр его не читает.
    let boxed = PAGE.replace("<p>a", "<p data-k=\"1\">a");
    assert_eq!(kept(PAGE, "p", "data-k", "1", &boxed), Some(false));
    let classed = PAGE.replace("<p>a", "<p class=\"zz\">a");
    assert_eq!(kept(PAGE, "p", "class", "zz", &classed), Some(false));
    // Элемент вне обхода сборки (`head`) не адресуется правкой: каскад заново.
    let head = PAGE.replace("<head>", "<head data-k=\"1\">");
    assert_eq!(kept(PAGE, "head", "data-k", "1", &head), None);
    // Упомянутый листом класс меняет каскад.
    let on = PAGE.replace("<p>a", "<p class=\"on\">a");
    assert_eq!(kept(PAGE, "p", "class", "on", &on), None);
}

#[test]
fn a_text_record_copies_its_path_into_a_new_frame() {
    let mut doc = Document::builder(PAGE, "").build().unwrap();
    let slot = doc.retained.clone();
    let p = element(&doc.source_dom(), "p").unwrap();
    let text = p.children.borrow()[0].clone();
    let record = Mutation::CharacterData {
        node: text,
        data: "z".into(),
    };
    doc.apply(&[record], "").unwrap();
    assert!(
        !Rc::ptr_eq(&slot, &doc.retained),
        "a changed text keeps no frame"
    );
    let fresh = Document::builder(&PAGE.replace("<p>a", "<p>z"), "")
        .build()
        .unwrap();
    assert_eq!(tree(&doc), tree(&fresh));
    assert_eq!(state(&doc), state(&fresh));
}

#[test]
fn a_clean_revision_still_answers_the_next_records_and_markup() {
    let mut doc = Document::builder(PAGE, "").build().unwrap();
    let html = element(&doc.source_dom(), "html").unwrap();
    doc.apply(&[attr(&html, "class", "alt")], "").unwrap();
    let classed = PAGE.replace("<html>", "<html class=\"alt\">");
    // Ключ ревизии новый: та же разметка — без изменений, другая — правка.
    let edited = classed.replace(">d<", ">e<");
    assert!(doc.update(&edited, "").unwrap().changed());
    let fresh = Document::builder(&edited, "").build().unwrap();
    assert_eq!(tree(&doc), tree(&fresh));
    assert_eq!(state(&doc), state(&fresh));
}
