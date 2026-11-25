//! Коробки перерассчитанных элементов в дереве и условия, при которых они и
//! их поддеревья принимают новые значения краски на месте.

use super::Restyle;
use crate::document::revision::boxes::{Boxes, at};
use crate::document::revision::patch::{Path, elements};
use crate::dom::{Element, Node};
use std::collections::HashMap;

/// The tree keeps its `body` box (under `html` and the viewport box).
pub(super) fn wrapped(nodes: &[Node]) -> bool {
    let mut level: Vec<&Node> = nodes.iter().collect();
    for _ in 0..3 {
        let mut next = Vec::new();
        for node in level {
            if let Node::Element(e) = node {
                if e.tag == "body" && e.node_id != 0 {
                    return true;
                }
                next.extend(e.children.iter());
            }
        }
        level = next;
    }
    false
}

/// Пути к коробкам перерассчитанных элементов (с их тегом и прежними
/// значениями краски) по индексу `boxes`, если каждая принимает новые
/// значения на месте (`takes`); у каждого элемента — хотя бы одна коробка.
pub(super) fn style_paths(
    nodes: &[Node],
    boxes: &Boxes,
    restyles: &HashMap<u64, Restyle>,
) -> Option<Vec<(Path, u64)>> {
    let mut paths = Vec::new();
    for (&id, r) in restyles {
        let before = paths.len();
        for path in boxes.paths(id) {
            let e = at(nodes, &path)?;
            if e.tag != r.tag {
                continue;
            }
            let cascaded = e.style.cascaded();
            let fits = match r.damage.0 {
                crate::style::computed::Damage::Paint => cascaded.same_paint_groups(&r.old),
                _ => cascaded.with_layout_of(&r.old, &r.new).is_some(),
            };
            if !(fits && takes(e, r)) {
                return None;
            }
            paths.push((path, id));
        }
        if paths.len() == before {
            return None;
        }
    }
    Some(paths)
}

/// The box `e` and its subtree take the restyle `r` of its element in place.
/// The box itself keeps no animation frames or copies of its computed values
/// outside the cascaded stage (for a new background, no decoration inputs
/// either; for new border colours, no inline box). Its subtree follows by
/// rematerialization: under new inherited
/// values no box keeps a first-letter/-line layer, animation frames
/// or such copies; under a new background alone only a style that inherits
/// reset values explicitly reads it (css-cascade-5 §7.3.1), and no such style
/// is a layer or keeps copies.
fn takes(e: &Element, r: &Restyle) -> bool {
    let (inherited, background, borders) = (r.inherited(), r.background(), r.borders());
    // A layout restyle drops the box's own used copy (`cascaded_mut`): the
    // next layout makes it again from the new values.
    let layout = r.damage.0 == crate::style::computed::Damage::Layout;
    let own = |b: &Element| match layout {
        true => b.style.holds_copies_but_used(),
        false => b.style.holds_inherited_copies(),
    };
    if e.anim.is_some() || own(e) {
        return false;
    }
    if (background || borders) && e.style.holds_decoration() {
        return false;
    }
    // The sides of an inline box and the collapsed borders of a table are
    // layout results with their colours.
    if borders && (e.inline || TABLE_PARTS.contains(&e.tag.as_str())) {
        return false;
    }
    elements(
        std::slice::from_ref(&Node::Element(e.clone())),
        |b, path| {
            let copies = b.anim.is_some()
                || if path.len() == 1 {
                    own(b)
                } else {
                    b.style.holds_inherited_copies()
                };
            if inherited {
                // A hover layer is a cascaded stage (the element's declarations
                // with its `:hover` rules over them, `walk::pseudo_styles`): it
                // keeps no inherited value and follows a new one like the box does.
                !copies && [&b.first_letter, &b.first_line].iter().all(|l| l.is_none())
            } else {
                !(copies && b.style.inherits_reset())
                    && layers(b).flatten().all(|l| !l.inherits_reset())
            }
        },
    )
}

/// The tags of the HTML table parts (HTML §15.3.8 gives them table displays).
const TABLE_PARTS: &[&str] = &[
    "table", "caption", "colgroup", "col", "thead", "tbody", "tfoot", "tr", "td", "th",
];

/// The hover and first-letter/-line layers of a box.
fn layers(b: &Element) -> impl Iterator<Item = &Option<crate::dom::SharedStyle>> {
    [&b.hover, &b.first_letter, &b.first_line].into_iter()
}

/// Whether the colours of the borders of a box with the cascaded style
/// `style` may be paint values: no table part, whose collapsed borders are a
/// layout result with their colours, and no inline-level box, whose sides are
/// painted with its line (`Computed::same_layout_but_border_colors`;
/// the box itself is checked too, `takes`).
pub(super) fn painted_borders(style: &crate::style::computed::Computed) -> bool {
    use crate::style::computed::Display;
    !matches!(
        style.reset_box_layout.display,
        Some(
            Display::Table
                | Display::InlineTable
                | Display::TableRow
                | Display::TableRowGroup
                | Display::TableCell
                | Display::Contents
                | Display::InlineBlock
                | Display::InlineFlex
                | Display::InlineGrid
        )
    )
}
