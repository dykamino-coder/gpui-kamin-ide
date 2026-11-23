//! Physical-row text damage is independent of CSSOM fragments. Until the
//! physical renderer tracks descendant ownership, only text leaves qualify.

use crate::document::boxes::BoxTree;
use crate::dom::{Element, Node};
use crate::layout::reuse::Store;
use gpui::{Bounds, ScaledPixels, point, size};

pub(super) fn area(
    store: &Store,
    boxes: &BoxTree,
    element: &Element,
    outset: f32,
) -> Option<Bounds<ScaledPixels>> {
    if !element.children.iter().all(|n| matches!(n, Node::Text(_))) {
        return None;
    }
    let id = boxes.box_of(element.node_id)?;
    let area = boxes.ancestors(id).find_map(|id| {
        boxes
            .slot(id)?
            .text_paint
            .as_ref()?
            .area(store, element.node_id)
    })?;
    let grow = outset * store.shown_scale();
    let area = Bounds {
        origin: point(
            ScaledPixels(area.origin.x.0 - grow),
            ScaledPixels(area.origin.y.0 - grow),
        ),
        size: size(
            ScaledPixels(area.size.width.0 + 2. * grow),
            ScaledPixels(area.size.height.0 + 2. * grow),
        ),
    };
    crate::layout::retained::finite_paint_bounds(&area).then_some(area)
}
