//! Сравнение исходных DOM двух ревизий одной формы.
//!
//! Форма — всё, кроме атрибутов элементов HTML вне шаблонов и чужого
//! содержимого: виды узлов, имена, тексты, комментарии, DOCTYPE, порядок и
//! число детей, содержимое шаблонов. Ревизия той же формы отличается только
//! такими атрибутами; их и перечисляет `Shape`. Любое другое отличие (текст,
//! дети, атрибут SVG/MathML или внутри `<template>`) — `None`: такая ревизия
//! собирается заново.

use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

/// Элемент, атрибуты которого в новой ревизии другие.
pub(super) struct Changed {
    pub(super) old: Handle,
    pub(super) new: Handle,
}

/// Отличия ревизии той же формы.
pub(super) struct Shape {
    pub(super) changed: Vec<Changed>,
    /// Адреса элементов прежнего и нового DOM в одном порядке обхода.
    pub(super) elements: Vec<(usize, usize)>,
    /// Тексты всех `<style>` нового DOM, включая содержимое шаблонов.
    pub(super) sheets: Vec<String>,
}

/// Отличия `new` от `old`, если у них одна форма.
pub(super) fn compare(old: &Handle, new: &Handle) -> Option<Shape> {
    let mut shape = Shape {
        changed: Vec::new(),
        elements: Vec::new(),
        sheets: Vec::new(),
    };
    // Узел, его пара и признак «атрибуты здесь — часть формы».
    let mut stack = vec![(old.clone(), new.clone(), false)];
    while let Some((a, b, frozen)) = stack.pop() {
        let frozen = match (&a.data, &b.data) {
            (NodeData::Document, NodeData::Document) => frozen,
            (
                NodeData::Doctype {
                    name,
                    public_id,
                    system_id,
                },
                NodeData::Doctype {
                    name: n,
                    public_id: p,
                    system_id: s,
                },
            ) if name == n && public_id == p && system_id == s => frozen,
            (NodeData::Text { contents }, NodeData::Text { contents: c })
                if *contents.borrow() == *c.borrow() =>
            {
                frozen
            }
            (NodeData::Comment { contents }, NodeData::Comment { contents: c })
                if contents == c =>
            {
                frozen
            }
            (
                NodeData::ProcessingInstruction { target, contents },
                NodeData::ProcessingInstruction {
                    target: t,
                    contents: c,
                },
            ) if target == t && contents == c => frozen,
            (
                NodeData::Element {
                    name,
                    attrs,
                    template_contents,
                    mathml_annotation_xml_integration_point: point,
                },
                NodeData::Element {
                    name: n,
                    attrs: at,
                    template_contents: t,
                    mathml_annotation_xml_integration_point: p,
                },
            ) => {
                if name != n || point != p {
                    return None;
                }
                // Чужое содержимое (SVG, MathML) и шаблоны обрабатываются своими
                // проходами, читающими атрибуты как есть: там они — часть формы.
                let frozen =
                    frozen || name.ns != html5ever::ns!(html) || &*name.local == "template";
                if *attrs.borrow() != *at.borrow() {
                    if frozen {
                        return None;
                    }
                    shape.changed.push(Changed {
                        old: a.clone(),
                        new: b.clone(),
                    });
                }
                shape
                    .elements
                    .push((Rc::as_ptr(&a) as usize, Rc::as_ptr(&b) as usize));
                if &*name.local == "style" {
                    shape.sheets.push(text_of(&b));
                }
                match (&*template_contents.borrow(), &*t.borrow()) {
                    (None, None) => {}
                    (Some(x), Some(y)) => stack.push((x.clone(), y.clone(), true)),
                    _ => return None,
                }
                frozen
            }
            _ => return None,
        };
        let (ka, kb) = (a.children.borrow(), b.children.borrow());
        if ka.len() != kb.len() {
            return None;
        }
        for (x, y) in ka.iter().zip(kb.iter()).rev() {
            stack.push((x.clone(), y.clone(), frozen));
        }
    }
    Some(shape)
}

fn text_of(node: &Handle) -> String {
    let mut out = String::new();
    for child in node.children.borrow().iter() {
        if let NodeData::Text { contents } = &child.data {
            out.push_str(&contents.borrow());
        }
    }
    out
}
