//! Pagination struts applied to a copy of the DOM before it is laid out again.

use super::{column_flex, layout_parent, row_flex};
use crate::dom::{Element, Node};
use crate::layout::fragment::clone::monolith_box;
use crate::style::computed::Display;
use crate::style::values::value::{Len, px_auto_zero};
use std::collections::HashMap;

/// A pagination strut: how far the box moves, and its laid-out border-box
/// block size before the first push.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Strut {
    pub(crate) by: f32,
    pub(crate) size: f32,
    /// The flex row gap before this item's row, carried as its margin so
    /// that one row's gap can be truncated (`row-gap` is one value).
    pub(crate) gap: f32,
}

/// Struts by `node_id`.
pub(crate) type Struts = HashMap<u64, Strut>;

/// Sum of the struts inside `k` (not its own).
fn inner_by(k: &Element, struts: &Struts) -> f32 {
    k.children
        .iter()
        .map(|n| match n {
            Node::Element(x) => struts.get(&x.node_id).map_or(0.0, |s| s.by) + inner_by(x, struts),
            _ => 0.0,
        })
        .sum()
}

/// Content block size for a border-box size `h` of `k` (lengths in px).
fn content_height(k: &Element, h: f32) -> Option<f32> {
    if k.style.border_box == Some(true) {
        return Some(h);
    }
    let b = k.style.borders();
    Some(
        h - px_auto_zero(k.style.padding.top)?
            - px_auto_zero(k.style.padding.bottom)?
            - px_auto_zero(b.top)?
            - px_auto_zero(b.bottom)?,
    )
}

/// Apply pagination struts to a copy of the DOM: a block parent gets an empty
/// spacer box before the pushed box; a flex or grid item gets the strut added
/// to its block-start margin, so the line or track grows with it. A stretched
/// item of a flex row keeps its pre-break size: the row expands to the pushed
/// item (Blink row expansion) instead of the margin eating the stretch.
/// Returns `false` when a strut cannot be expressed.
pub(crate) fn apply(c: &mut Element, struts: &Struts) -> bool {
    if struts.is_empty() {
        return true;
    }
    let as_margin = layout_parent(&c.style);
    let row = row_flex(&c.style);
    // A column flex keeps every item's main size from before fragmentation;
    // an item may grow to encompass its fragmented content (Blink
    // `MinBlockSizeShouldEncompassIntrinsicSize`, flex_layout_algorithm.cc).
    let column = column_flex(&c.style);
    let can_shrink_box = matches!(c.style.height, Some(Len::Px(_)));
    let moved = column && inner_by(c, struts) > 0.0;
    // Row gaps carried by the items' margins (`Strut::gap`).
    let gap_rows = c.children.iter().any(
        |n| matches!(n, Node::Element(k) if struts.get(&k.node_id).is_some_and(|s| s.gap > 0.0)),
    );
    if gap_rows && let Some((_, col)) = c.style.gap {
        c.style.gap = Some((Some(Len::Px(0.0)), col));
    }
    let mut i = 0;
    while i < c.children.len() {
        let mut spacer = None;
        if let Node::Element(k) = &mut c.children[i] {
            if column && let Some(st) = struts.get(&k.node_id) {
                let can_shrink = can_shrink_box && k.style.flex_shrink != Some(0.0);
                let auto_h = matches!(k.style.height, None | Some(Len::Auto));
                let basis_auto = matches!(k.style.flex_basis, None | Some(Len::Auto)) && auto_h;
                let encompass = !monolith_box(k)
                    && ((basis_auto && !can_shrink)
                        || (auto_h && (!can_shrink || k.style.min_height.is_none())));
                // A definite block size stays: pushed content overflows it as
                // in the unfragmented flow thread.
                let grown = st.size + if auto_h { inner_by(k, struts) } else { 0.0 };
                let Some(h) = content_height(k, grown) else {
                    return false;
                };
                if encompass {
                    k.style.height = None;
                    k.style.min_height = Some(Len::Px(h.max(0.0)));
                } else if moved {
                    k.style.height = Some(Len::Px(h.max(0.0)));
                    k.style.min_height = None;
                }
                if encompass || moved {
                    k.style.max_height = None;
                    k.style.flex_basis = None;
                    k.style.flex_grow = Some(0.0);
                    k.style.flex_shrink = Some(0.0);
                }
            }
            if let Some(&st) = struts.get(&k.node_id).filter(|s| s.gap > 0.0 && gap_rows) {
                let Some(m) = px_auto_zero(k.style.margin.top) else {
                    return false;
                };
                k.style.margin.top = Some(Len::Px(m + st.gap));
            }
            if let Some(&st) = struts.get(&k.node_id).filter(|s| s.by != 0.0) {
                if as_margin {
                    let Some(m) = px_auto_zero(k.style.margin.top) else {
                        return false;
                    };
                    k.style.margin.top = Some(Len::Px(m + st.by));
                    if row && matches!(k.style.height, None | Some(Len::Auto)) {
                        let Some(h) = content_height(k, st.size) else {
                            return false;
                        };
                        k.style.height = Some(Len::Px(h.max(0.0)));
                    }
                } else if st.by > 0.0 {
                    spacer = Some(spacer_box(k.node_id, st.by));
                }
            }
            if !apply(k, struts) {
                return false;
            }
        }
        if let Some(sp) = spacer {
            c.children.insert(i, Node::Element(sp));
            i += 1;
        }
        i += 1;
    }
    true
}

fn spacer_box(id: u64, h: f32) -> Element {
    Element {
        marks: Default::default(),
        list_item: None,
        node_id: id ^ 0x5041_4749_4E41_5445,
        anim: None,
        tag: "div".to_string(),
        style: crate::style::computed::Computed {
            display: Some(Display::Block),
            height: Some(Len::Px(h)),
            ..Default::default()
        },
        hover: None,
        first_letter: None,
        first_line: None,
        children: Vec::new(),
        attrs: Vec::new(),
        inline: false,
    }
}
