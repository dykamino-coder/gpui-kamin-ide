//! Value equality of box trees across document revisions and frames.
//!
//! Layout passes copy the nodes they rewrite (`Children` is copy-on-write), so
//! the same subtree of two frames is often a different allocation with the
//! same values. Shared child lists and style stages compare by pointer first;
//! the walk is iterative, as deep trees are legal input.

use super::{Children, Element, Node, SharedStyle};
use std::rc::Rc;

/// Whether two elements carry the same values apart from their children.
pub fn same_box(a: &Element, b: &Element) -> bool {
    a.node_id == b.node_id && same_values(a, b)
}

/// Whether two elements carry the same values apart from their children and
/// their node ids.
pub fn same_values(a: &Element, b: &Element) -> bool {
    same_shape(a, b) && same_styles(a, b)
}

/// The values of two elements that compare in a few words: everything but
/// marks and style stages.
fn same_shape(a: &Element, b: &Element) -> bool {
    same_kind(a, b) && a.attrs == b.attrs
}

/// `same_shape` without the attributes.
fn same_kind(a: &Element, b: &Element) -> bool {
    let anim = match (&a.anim, &b.anim) {
        (None, None) => true,
        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
        _ => false,
    };
    anim && a.list_item == b.list_item && a.inline == b.inline && a.tag == b.tag
}

/// Whether two elements give their children the same layout inputs apart
/// from their children: equal but for paint-only style values
/// (`Computed::same_layout`) and for `class` and `data-*` attributes, which
/// only the cascade reads (an HTML root box is never inside SVG, whose
/// serialization would carry them).
pub fn same_layout_box(a: &Element, b: &Element) -> bool {
    let read = |(name, _): &&(String, String)| name != "class" && !name.starts_with("data-");
    a.node_id == b.node_id
        && same_kind(a, b)
        && (a.attrs == b.attrs || a.attrs.iter().filter(read).eq(b.attrs.iter().filter(read)))
        && a.marks == b.marks
        && BoxStyles::same_layout_as(&a.style, layers(a), b)
}

/// Whether the subtree of `b` lays out like the recorded one of `same_tree_as`:
/// the same values throughout but for paint-only style values
/// (`Computed::same_layout`). Attributes and texts compare exactly.
pub fn same_layout_tree_as(
    shell: &Element,
    styles: &BoxStyles,
    children: &Children,
    b: &Element,
) -> bool {
    let root = || shell.marks == b.marks && styles.same_layout(b);
    walk(same_shape(shell, b), children, b, same_shape)
        && walk(root(), children, b, same_layout_styles)
}

fn same_layout_styles(a: &Element, b: &Element) -> bool {
    a.marks == b.marks && BoxStyles::same_layout_as(&a.style, layers(a), b)
}

/// Whether two elements carry the same values apart from their children,
/// node ids and style stages: boxes of one kind in a flow share these.
pub fn same_shell(a: &Element, b: &Element) -> bool {
    same_shape(a, b) && a.marks == b.marks
}

/// Marks and style stages, which may compare field by field.
fn same_styles(a: &Element, b: &Element) -> bool {
    a.marks == b.marks && BoxStyles::same_as(&a.style, layers(a), b)
}

fn layers(e: &Element) -> [Option<&SharedStyle>; 3] {
    [&e.hover, &e.first_letter, &e.first_line].map(Option::as_ref)
}

/// The style stages of one box: its style and the pseudo-element layers,
/// which most boxes do not have. Shared stages are pointers into the
/// styles of the build they were taken from.
#[derive(Clone)]
pub struct BoxStyles {
    style: SharedStyle,
    layers: Option<Box<[Option<SharedStyle>; 3]>>,
}

impl BoxStyles {
    pub fn of(e: &Element) -> Self {
        let layers = layers(e).map(Option::<&SharedStyle>::cloned);
        let layers = layers.iter().any(Option::is_some).then(|| Box::new(layers));
        let style = e.style.clone();
        Self { style, layers }
    }

    /// The element `shell` with these style stages.
    pub fn apply(&self, shell: &Element) -> Element {
        let [hover, first_letter, first_line] = self.layers.as_deref().cloned().unwrap_or_default();
        Element {
            style: self.style.clone(),
            hover,
            first_letter,
            first_line,
            ..shell.clone()
        }
    }

    fn same(&self, b: &Element) -> bool {
        let layers = self
            .layers
            .as_deref()
            .map(|l| l.each_ref().map(Option::as_ref));
        Self::same_as(&self.style, layers.unwrap_or_default(), b)
    }

    fn same_as(style: &SharedStyle, own: [Option<&SharedStyle>; 3], b: &Element) -> bool {
        let layer = |x: Option<&SharedStyle>, y: Option<&SharedStyle>| match (x, y) {
            (None, None) => true,
            (Some(x), Some(y)) => x.same(y),
            _ => false,
        };
        style.same(&b.style) && own.into_iter().zip(layers(b)).all(|(x, y)| layer(x, y))
    }

    fn same_layout(&self, b: &Element) -> bool {
        let layers = self
            .layers
            .as_deref()
            .map(|l| l.each_ref().map(Option::as_ref));
        Self::same_layout_as(&self.style, layers.unwrap_or_default(), b)
    }

    fn same_layout_as(style: &SharedStyle, own: [Option<&SharedStyle>; 3], b: &Element) -> bool {
        let layer = |x: Option<&SharedStyle>, y: Option<&SharedStyle>| match (x, y) {
            (None, None) => true,
            (Some(x), Some(y)) => x.same_layout(y),
            _ => false,
        };
        style.same_layout(&b.style) && own.into_iter().zip(layers(b)).all(|(x, y)| layer(x, y))
    }
}

/// Whether two subtrees carry the same values throughout.
pub fn same_tree(a: &Element, b: &Element) -> bool {
    a.node_id == b.node_id
        && walk(same_shape(a, b), &a.children, b, same_shape)
        && walk(same_styles(a, b), &a.children, b, same_styles)
}

/// Whether the subtree of `b` carries throughout the values of a root box
/// with the values of `shell` apart from node id, children and style stages
/// (`same_shell`), the style stages `styles` and the children `children`.
/// A host that keeps many subtrees for comparison shares one shell between
/// boxes of one kind; their style stages differ (a style knows its node).
///
/// A revised document restyles every box into new style allocations, so equal
/// styles compare field by field. The first walk compares the structure, the
/// texts and the few-word values and stops at a changed text or attribute;
/// only a subtree equal in all of them is walked again for its styles.
pub fn same_tree_as(shell: &Element, styles: &BoxStyles, children: &Children, b: &Element) -> bool {
    let root = || shell.marks == b.marks && styles.same(b);
    walk(same_shape(shell, b), children, b, same_shape) && walk(root(), children, b, same_styles)
}

/// The roots are equal (`root`), every pair of their descendants passes
/// `same` and has the same node id, and their texts are equal.
fn walk<'a>(
    root: bool,
    children: &'a Children,
    b: &'a Element,
    mut same: impl FnMut(&'a Element, &'a Element) -> bool,
) -> bool {
    let mut pending: Vec<(&'a Element, &'a Element)> = Vec::new();
    if !root || !same_children(children, &b.children, &mut pending) {
        return false;
    }
    while let Some((x, y)) = pending.pop() {
        if x.node_id != y.node_id
            || !same(x, y)
            || !same_children(&x.children, &y.children, &mut pending)
        {
            return false;
        }
    }
    true
}

/// Compare the child lists `a` and `b` up to their element children, which
/// go to `pending`.
fn same_children<'a>(
    a: &'a Children,
    b: &'a Children,
    pending: &mut Vec<(&'a Element, &'a Element)>,
) -> bool {
    if a.identity() == b.identity() {
        return true;
    }
    if a.len() != b.len() {
        return false;
    }
    for pair in a.iter().zip(b.iter()) {
        match pair {
            (Node::Text(x), Node::Text(y)) => {
                if x.as_str() != y.as_str() || x.source() != y.source() {
                    return false;
                }
            }
            (Node::Element(x), Node::Element(y)) => pending.push((x, y)),
            _ => return false,
        }
    }
    true
}

mod cascaded;
pub use cascaded::{CascadedMemo, same_cascaded_box, same_cascaded_tree};
mod unread_color;
pub use unread_color::{paints_without_color, same_unread_color_tree_as};

#[cfg(test)]
mod tests;
