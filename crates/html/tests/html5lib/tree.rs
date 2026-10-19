//! Build and serialize HTML documents and fragments in html5lib tree notation.

use super::cases::Case;
use html5ever::driver::ParseOpts;
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{QualName, ns, parse_document, parse_fragment};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

/// Имя контекстного элемента куска: `svg path` — это `path` в чужом
/// пространстве имён.
fn context_name(spec: &str) -> QualName {
    let mut parts = spec.split_whitespace();
    let (ns, local) = match (parts.next(), parts.next()) {
        (Some("svg"), Some(local)) => (ns!(svg), local),
        (Some("math"), Some(local)) => (ns!(mathml), local),
        (Some(local), None) => (ns!(html), local),
        (Some(_), Some(local)) => (ns!(html), local),
        _ => (ns!(html), "div"),
    };
    QualName::new(None, ns, local.into())
}

/// Дерево в формате набора: отступ два пробела на уровень, префикс `| `.
fn serialize(node: &Handle, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match &node.data {
        NodeData::Document => {}
        NodeData::Doctype {
            name,
            public_id,
            system_id,
        } => {
            out.push_str(&format!("| {pad}<!DOCTYPE {name}"));
            if !public_id.is_empty() || !system_id.is_empty() {
                out.push_str(&format!(" \"{public_id}\" \"{system_id}\""));
            }
            out.push_str(">\n");
        }
        NodeData::Text { contents } => {
            out.push_str(&format!("| {pad}\"{}\"\n", contents.borrow()));
        }
        NodeData::Comment { contents } => {
            out.push_str(&format!("| {pad}<!-- {contents} -->\n"));
        }
        NodeData::ProcessingInstruction { target, contents } => {
            out.push_str(&format!(
                "| {pad}<?{target} {contents}?>
"
            ));
        }
        NodeData::Element {
            name,
            attrs,
            template_contents,
            ..
        } => {
            let prefix = match name.ns {
                ns!(svg) => "svg ",
                ns!(mathml) => "math ",
                _ => "",
            };
            out.push_str(&format!("| {pad}<{prefix}{}>\n", name.local));
            // Атрибуты набор печатает отсортированными по имени.
            let mut list: Vec<(String, String)> = attrs
                .borrow()
                .iter()
                .map(|a| {
                    let name = match (&a.name.prefix, a.name.ns.clone()) {
                        (Some(p), _) => format!("{p} {}", a.name.local),
                        _ => a.name.local.to_string(),
                    };
                    (name, a.value.to_string())
                })
                .collect();
            list.sort();
            for (attr, value) in list {
                out.push_str(&format!("| {}  {attr}=\"{value}\"\n", "  ".repeat(depth)));
            }
            // Содержимое `<template>` — отдельный узел `content`.
            if let Some(contents) = template_contents.borrow().as_ref() {
                out.push_str(&format!("| {}  content\n", "  ".repeat(depth)));
                for child in contents.children.borrow().iter() {
                    serialize(child, depth + 2, out);
                }
            }
        }
    }
    let next = match node.data {
        NodeData::Document => depth,
        _ => depth + 1,
    };
    for child in node.children.borrow().iter() {
        serialize(child, next, out);
    }
}

/// Разобрать вход и напечатать дерево так же, как это делает набор.
pub(super) fn tree_of(case: &Case, scripting: bool) -> String {
    let opts = ParseOpts {
        tree_builder: TreeBuilderOpts {
            scripting_enabled: scripting,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut out = String::new();
    match &case.fragment {
        Some(spec) => {
            let dom = parse_fragment(
                RcDom::default(),
                opts,
                context_name(spec),
                vec![],
                // Разбор куска идёт в дереве без документа: у html5ever это
                // отдельный вход, и стартовый контекст задаёт имя элемента.
                // Последний признак — разрешены ли сценарии в контексте.
                scripting,
            )
            .one(case.data.clone());
            // Первый ребёнок документа — служебный `<html>`, дети которого и
            // есть разобранный кусок.
            let root = dom.document.children.borrow()[0].clone();
            for child in root.children.borrow().iter() {
                serialize(child, 0, &mut out);
            }
        }
        None => {
            let dom = parse_document(RcDom::default(), opts).one(case.data.clone());
            serialize(&dom.document, 0, &mut out);
        }
    }
    out
}
