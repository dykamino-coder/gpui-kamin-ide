//! Закреплённые абсолютные потомки сохраняют свой containing block вне вложенных band.

use super::lift::abs_pinned;
use crate::dom::{Element, Node};
use crate::layout::float::band_host::band_em;
use crate::layout::float::band_nest::band_nest_block;
use crate::style::computed::writing_mode::WritingMode;

/// CSS 2.1 sections 10.1 and 10.3.7/10.6.4: an unpositioned flow block
/// does not establish the absolute containing block. A descendant with an
/// inset on each axis can use the same lift as a direct pinned child;
/// neither its hypothetical line nor the nested float geometry positions it.
/// Stop at every box outside the shared nested flow contract, including
/// positioned ancestors and independent formatting contexts.
pub(super) fn lift_nested(c: &mut Element, em: f32, wm: WritingMode, lift: &mut Vec<Node>) {
    if !band_nest_block(c, em, wm) {
        return;
    }
    let Some(inner_em) = band_em(&c.style.used(), em) else {
        return;
    };
    c.children = std::mem::take(&mut c.children)
        .into_iter()
        .filter_map(|mut node| {
            if let Node::Element(child) = &mut node {
                if abs_pinned(&child.style.used()) {
                    lift.push(node);
                    return None;
                }
                lift_nested(child, inner_em, wm, lift);
            }
            Some(node)
        })
        .collect();
}
