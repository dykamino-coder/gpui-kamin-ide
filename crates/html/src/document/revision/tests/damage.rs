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
/// ревизия свежей сборке `new`, и сохранены ли дерево, кадр и состояние.
fn kept(old: &str, local: &str, name: &str, value: &str, new: &str) -> bool {
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
    let same = Rc::ptr_eq(&nodes, &doc.nodes);
    assert_eq!(
        same,
        Rc::ptr_eq(&slot, &doc.retained),
        "tree and frame go together"
    );
    assert_eq!(
        same,
        Rc::ptr_eq(&state_rc, &doc.state),
        "tree and state go together"
    );
    same
}

#[test]
fn records_on_boxless_elements_keep_the_tree_the_frame_and_the_state() {
    let classed = PAGE.replace("<html>", "<html class=\"alt\">");
    assert!(kept(PAGE, "html", "class", "alt", &classed));
    let head = PAGE.replace("<head>", "<head data-k=\"1\">");
    assert!(kept(PAGE, "head", "data-k", "1", &head));
    // Первый `p` — коробка: правка копирует путь к ней.
    let boxed = PAGE.replace("<p>a", "<p data-k=\"1\">a");
    assert!(!kept(PAGE, "p", "data-k", "1", &boxed));
    let hidden = PAGE.replace("<p>d</p>", "<p>d</p><p style=\"display:none\">h</p>");
    let none = hidden.replace("display:none\"", "display:none\" class=\"zz\"");
    let mut doc = Document::builder(&hidden, "").build().unwrap();
    let nodes = doc.nodes.clone();
    let body = element(&doc.source_dom(), "body").unwrap();
    let target = body.children.borrow().last().cloned().unwrap();
    doc.apply(&[attr(&target, "class", "zz")], "").unwrap();
    assert!(Rc::ptr_eq(&nodes, &doc.nodes), "display:none has no box");
    let fresh = Document::builder(&none, "").build().unwrap();
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
