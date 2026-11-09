//! Сравнение исходных DOM двух ревизий одной формы.
//!
//! Форма — всё, кроме атрибутов элементов HTML и текстов вне шаблонов и
//! чужого содержимого: виды узлов, имена, комментарии, DOCTYPE, порядок и
//! число детей, содержимое шаблонов. Ревизия той же формы отличается только
//! такими атрибутами и текстами; их и перечисляет `Shape`. Любое другое
//! отличие (дети, атрибут или текст SVG/MathML или внутри `<template>`) —
//! `None`: такая ревизия собирается заново.

use super::mutation::{Mutation, attribute_records};
use markup5ever_rcdom::{Handle, NodeData};

/// Элемент, атрибуты которого в новой ревизии другие.
pub(super) struct Changed {
    pub(super) old: Handle,
    pub(super) new: Handle,
}

/// Текстовый узел элемента HTML, содержимое которого в новой ревизии другое.
pub(super) struct Edited {
    /// Текстовый узел прежнего DOM.
    pub(super) node: Handle,
    pub(super) new: String,
}

/// Отличия ревизии той же формы.
pub(super) struct Shape {
    pub(super) changed: Vec<Changed>,
    pub(super) texts: Vec<Edited>,
    /// Тексты всех `<style>` нового DOM, включая содержимое шаблонов.
    pub(super) sheets: Vec<String>,
}

impl Shape {
    /// Записи мутаций, переводящие прежний DOM в новый: атрибуты элементов и
    /// содержимое текстов, адресованные узлам прежнего DOM.
    pub(super) fn records(&self) -> Vec<Mutation> {
        let attrs = |node: &Handle| match &node.data {
            NodeData::Element { attrs, .. } => attrs.borrow().clone(),
            _ => Vec::new(),
        };
        let mut out = Vec::new();
        for changed in &self.changed {
            out.extend(attribute_records(
                &changed.old,
                &attrs(&changed.old),
                &attrs(&changed.new),
            ));
        }
        out.extend(self.texts.iter().map(|edit| Mutation::CharacterData {
            node: edit.node.clone(),
            data: edit.new.clone(),
        }));
        out
    }
}

/// Отличия `new` от `old`, если у них одна форма.
pub(super) fn compare(old: &Handle, new: &Handle) -> Option<Shape> {
    let mut shape = Shape {
        changed: Vec::new(),
        texts: Vec::new(),
        sheets: Vec::new(),
    };
    // Узел, его пара, признак «атрибуты и тексты здесь — часть формы» и
    // есть ли у узла родитель.
    let mut stack = vec![(old.clone(), new.clone(), false, false)];
    while let Some((a, b, frozen, child)) = stack.pop() {
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
            (NodeData::Text { contents }, NodeData::Text { contents: c }) => {
                if *contents.borrow() != *c.borrow() {
                    if frozen || !child {
                        return None;
                    }
                    shape.texts.push(Edited {
                        node: a.clone(),
                        new: c.borrow().to_string(),
                    });
                }
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
                if &*name.local == "style" {
                    shape.sheets.push(text_of(&b));
                }
                match (&*template_contents.borrow(), &*t.borrow()) {
                    (None, None) => {}
                    (Some(x), Some(y)) => stack.push((x.clone(), y.clone(), true, false)),
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
            stack.push((x.clone(), y.clone(), frozen, true));
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

/// Тексты всех `<style>` DOM `root`, включая содержимое шаблонов, в порядке
/// обхода (как `Shape::sheets`).
pub(super) fn sheets(root: &Handle) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(node) = stack.pop() {
        if let NodeData::Element {
            name,
            template_contents,
            ..
        } = &node.data
        {
            if &*name.local == "style" {
                out.push(text_of(&node));
            }
            if let Some(contents) = &*template_contents.borrow() {
                stack.push(contents.clone());
            }
        }
        stack.extend(node.children.borrow().iter().rev().cloned());
    }
    out
}
