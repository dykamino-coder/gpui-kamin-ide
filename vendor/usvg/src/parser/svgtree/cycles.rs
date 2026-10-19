// Copyright 2026 the KaminIDE Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Resource reference cycle breaking (KaminIDE addition).
//!
//! Upstream usvg only detected cycles of length one or two per resource kind
//! (`fix_recursive_links`/`fix_recursive_patterns`), so a chain like
//! `clip -> clip2 -> clip0 -> clip` (WPT css-masking `clip-path-recursion-001`)
//! or a mixed `clipPath -> mask -> clipPath` loop made the converter recurse
//! until the stack overflowed.
//!
//! This follows Blink's `SVGResource::FindCycle` /
//! `LayoutSVGResourceContainer::FindCycleInResources`: one graph over every
//! resource kind (clipPath, mask, pattern, filter, marker), a depth-first
//! search for back-edges starting from the rendered content in document order,
//! and the reference that closes a cycle is made invalid. usvg models an
//! invalid reference as `none`, like the upstream fixes did.

use std::collections::HashMap;

use super::{AId, Document, EId, NodeId, SvgNode};

/// Attributes whose `url(#id)` references make the converter descend into
/// another resource's content.
const LINK_ATTRS: [AId; 8] = [
    AId::ClipPath,
    AId::Mask,
    AId::Filter,
    AId::Fill,
    AId::Stroke,
    AId::MarkerStart,
    AId::MarkerMid,
    AId::MarkerEnd,
];

fn is_resource(eid: Option<EId>) -> bool {
    matches!(
        eid,
        Some(EId::ClipPath | EId::Mask | EId::Pattern | EId::Filter | EId::Marker)
    )
}

/// Every `url(#id)` in an attribute value (a filter list or a paint with a
/// fallback colour may hold more than one function).
fn url_ids(value: &str) -> impl Iterator<Item = &str> {
    value.split("url(").skip(1).filter_map(|rest| {
        let end = rest.find(')')?;
        let inner = rest[..end].trim().trim_matches(|c| c == '"' || c == '\'');
        inner.strip_prefix('#')
    })
}

/// A reference held by `holder` in attribute `aid` to resource `target`.
#[derive(Clone, Copy)]
struct Edge {
    holder: NodeId,
    aid: AId,
    target: NodeId,
}

fn node_edges(node: SvgNode, out: &mut Vec<Edge>) {
    for aid in LINK_ATTRS {
        let Some(value) = node.attribute::<&str>(aid) else {
            continue;
        };
        for id in url_ids(value) {
            if let Some(target) = node.document().element_by_id(id) {
                if is_resource(target.tag_name()) {
                    out.push(Edge {
                        holder: node.id,
                        aid,
                        target: target.id,
                    });
                }
            }
        }
    }
}

/// `root` and its descendants, skipping nested resources: those are reached
/// only through a reference (Blink `FindCycleInDescendants`).
fn subtree_edges(root: SvgNode, out: &mut Vec<Edge>) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.id != root.id && is_resource(node.tag_name()) {
            continue;
        }
        node_edges(node, out);
        let mut kids: Vec<_> = node.children().collect();
        kids.reverse();
        stack.extend(kids);
    }
}

/// References followed while converting resource `res`: its own attributes
/// (`clip-path` on `<clipPath>`, `mask` on `<mask>`), its content (for a
/// pattern — the first content in its `href` chain) and, for a filter, the
/// elements its `feImage` primitives render.
fn resource_edges(res: SvgNode) -> Vec<Edge> {
    let mut out = Vec::new();
    let content = if res.tag_name() == Some(EId::Pattern) {
        res.href_iter().find(|n| n.has_children()).unwrap_or(res)
    } else {
        res
    };
    node_edges(res, &mut out);
    for kid in content.children() {
        subtree_edges(kid, &mut out);
    }
    if res.tag_name() == Some(EId::Filter) {
        for prim in res.children() {
            if prim.tag_name() == Some(EId::FeImage) {
                if let Some(link) = prim.node_attribute(AId::Href) {
                    if !is_resource(link.tag_name()) {
                        subtree_edges(link, &mut out);
                    }
                }
            }
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Mark {
    Active,
    Done,
}

/// Iterative DFS from `start`; back-edges are collected into `broken`.
fn visit(
    doc: &Document,
    start: NodeId,
    marks: &mut HashMap<u32, Mark>,
    broken: &mut Vec<(NodeId, AId)>,
) {
    if marks.contains_key(&start.get()) {
        return;
    }
    marks.insert(start.get(), Mark::Active);
    let mut stack = vec![(start, resource_edges(doc.get(start)), 0usize)];
    while let Some((res, edges, next)) = stack.last_mut() {
        let Some(edge) = edges.get(*next).copied() else {
            marks.insert(res.get(), Mark::Done);
            stack.pop();
            continue;
        };
        *next += 1;
        match marks.get(&edge.target.get()) {
            Some(Mark::Active) => broken.push((edge.holder, edge.aid)),
            Some(Mark::Done) => {}
            None => {
                marks.insert(edge.target.get(), Mark::Active);
                let target_edges = resource_edges(doc.get(edge.target));
                stack.push((edge.target, target_edges, 0));
            }
        }
    }
}

pub(super) fn fix_resource_cycles(doc: &mut Document) {
    let mut marks = HashMap::new();
    let mut broken = Vec::new();
    // Rendered content first, in document order, so the reference that
    // closes a cycle is the one Blink would invalidate; then resources
    // referenced only from other resources or not at all.
    let mut roots = Vec::new();
    subtree_edges(doc.root(), &mut roots);
    for edge in roots {
        visit(doc, edge.target, &mut marks, &mut broken);
    }
    let resources: Vec<NodeId> = doc
        .root()
        .descendants()
        .filter(|n| is_resource(n.tag_name()))
        .map(|n| n.id)
        .collect();
    for res in resources {
        visit(doc, res, &mut marks, &mut broken);
    }
    for (holder, aid) in broken {
        if let Some(idx) = doc.get(holder).attribute_id(aid) {
            doc.attrs[idx].value = roxmltree::StringStorage::Borrowed("none");
        }
    }
}
