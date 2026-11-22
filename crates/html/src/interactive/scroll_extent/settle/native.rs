//! Children of a scrollport that only the native layout places, and the area of a CSS
//! box's descendants through its ancestors' clips, as `Frame::settle` measures them.

use super::super::classic;
use super::super::geometry::{Clip, Rect, union};
use crate::dom::{Element, Node};
use gpui::{Bounds, Pixels, ScrollHandle};
use std::collections::HashMap;

/// How far snapping can move a native child's far edge from its exact one.
const SNAPPED: f32 = 1.0;

pub(super) fn area(
    e: &Element,
    boxes: &HashMap<u64, (Rect, Clip)>,
    text: &HashMap<u64, Rect>,
) -> Option<Rect> {
    let descendants = e
        .children
        .iter()
        .filter_map(|node| match node {
            Node::Element(child) => area(child, boxes, text),
            _ => None,
        })
        .chain(text.get(&e.node_id).copied())
        .reduce(union);
    let Some(&(own, clip)) = boxes.get(&e.node_id) else {
        return descendants;
    };
    Some(
        descendants
            .and_then(|r| clip.intersect(own, r))
            .map_or(own, |r| union(own, r)),
    )
}

/// Native children reach the range's far end (the proxy is child zero); a positioned
/// child already measured, and a decoration, add nothing.
#[allow(clippy::too_many_arguments)]
pub(super) fn reach_native(
    tree: &Element,
    handle: &ScrollHandle,
    bounds: Bounds<Pixels>,
    gutter: [f32; 2],
    origin: [f32; 2],
    padding: [f32; 4],
    positioned: &[[f32; 4]],
    end: &mut [f32; 2],
    reach: &mut impl FnMut(&mut [f32; 2], usize, f32, (&'static str, u64)),
) {
    // The proxy is child zero. Native children also cover text-only runs
    // and CSS boxes whose own nested scrollport owns their descendants.
    if tree
        .children
        .iter()
        .any(|n| matches!(n, Node::Text(t) if !crate::dom::whitespace::blank_text(t)))
        && let Some(marker) = handle.bounds_for_item(0)
    {
        for index in 1..handle.children_count() {
            if let Some(child) = handle.bounds_for_item(index)
                && !classic::decoration(child, bounds, gutter)
            {
                let at = [
                    origin[0] + f32::from(child.left() - marker.left()),
                    origin[1] + f32::from(child.top() - marker.top()),
                    origin[0] + f32::from(child.right() - marker.left()),
                    origin[1] + f32::from(child.bottom() - marker.top()),
                ];
                // A positioned child box was measured above, without the
                // end padding of the in-flow content.
                if positioned
                    .iter()
                    .any(|r: &[f32; 4]| r.iter().zip(at).all(|(a, b)| (a - b).abs() <= SNAPPED))
                {
                    continue;
                }
                // Native bounds are snapped to whole pixels, layout's are not: a
                // native child within a pixel of the range is that snapping, not
                // overflow layout missed (it decides the bars from exact bounds).
                let edge = [at[2] + padding[2], at[3] + padding[3]];
                for axis in 0..2 {
                    if edge[axis] > end[axis] + SNAPPED {
                        reach(end, axis, edge[axis], ("a native child", index as u64));
                    }
                }
            }
        }
    }
}
