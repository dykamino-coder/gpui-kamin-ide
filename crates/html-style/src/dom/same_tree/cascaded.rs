//! Value equality of two cascades of one source DOM by their cascaded values.
//!
//! A computed style is a function of the element's cascaded style, its
//! parent's computed style and the document inputs (viewport, fonts). Two
//! cascades of the same source DOM whose subtrees carry equal cascaded values
//! under equal parents therefore compute equal values throughout, so a
//! rebuild can compare them before its styles are computed (Blink
//! `StyleDifference` compares the old and new style of each object at
//! recalc, before layout). Paint-only values of an ancestor reach the
//! subtree only as paint-only values (`Computed::same_layout`).
//!
//! The cascade shares the groups of equal inputs (the document's initial
//! groups across revisions, `StyleSharing` within one), so a pair of styles
//! is decided by its group owners (`Computed::shares_storage`): thousands of
//! blocks of a few kinds compare their values a few times (Gecko compares
//! style structs by pointer before by value, `nsStyleStruct::CalcDifference`).

use super::{Element, SharedStyle, layers, same_shape, walk};
use crate::style::computed::Computed;

/// Verdicts of the pairs of cascaded styles of one comparison of two trees,
/// by the group owners of each side; the trees outlive it.
#[derive(Default)]
pub struct CascadedMemo<'a>(Vec<Verdict<'a>>);

struct Verdict<'a> {
    a: &'a Computed,
    b: &'a Computed,
    layout: bool,
    same: bool,
}

/// Pairs kept: a flow repeats a few kinds of boxes.
const RECENT: usize = 16;

impl<'a> CascadedMemo<'a> {
    /// `x` and `y` are equal (`layout`: but for paint-only values).
    fn same(&mut self, x: &'a Computed, y: &'a Computed, layout: bool) -> bool {
        if std::ptr::eq(x, y) {
            return true;
        }
        // The one value outside the groups that differs per box.
        if x.decor_box != y.decor_box {
            return false;
        }
        let known = self
            .0
            .iter()
            .rposition(|v| v.layout == layout && v.a.shares_storage(x) && v.b.shares_storage(y));
        if let Some(at) = known {
            let verdict = self.0.remove(at);
            let same = verdict.same;
            self.0.push(verdict);
            return same;
        }
        let same = if layout { x.same_layout(y) } else { x == y };
        if self.0.len() == RECENT {
            self.0.remove(0);
        }
        self.0.push(Verdict {
            a: x,
            b: y,
            layout,
            same,
        });
        same
    }
}

/// The element styles of `a` and `b` cascade equal values (`layout`: equal
/// but for paint-only values), in every style stage.
pub fn same_cascaded_box<'a>(
    a: &'a Element,
    b: &'a Element,
    layout: bool,
    memo: &mut CascadedMemo<'a>,
) -> bool {
    let mut style =
        |x: &'a SharedStyle, y: &'a SharedStyle| memo.same(x.cascaded(), y.cascaded(), layout);
    a.marks == b.marks
        && style(&a.style, &b.style)
        && layers(a).into_iter().zip(layers(b)).all(|pair| match pair {
            (None, None) => true,
            (Some(x), Some(y)) => style(x, y),
            _ => false,
        })
}

/// The subtrees of `a` and `b` have the same node ids, structure, texts and
/// attributes, and cascade equal values throughout (`layout`: equal but for
/// paint-only values; see the module).
pub fn same_cascaded_tree<'a>(
    a: &'a Element,
    b: &'a Element,
    layout: bool,
    memo: &mut CascadedMemo<'a>,
) -> bool {
    a.node_id == b.node_id
        && same_shape(a, b)
        && same_cascaded_box(a, b, layout, memo)
        && walk(true, &a.children, b, |x, y| {
            same_shape(x, y) && same_cascaded_box(x, y, layout, memo)
        })
}
