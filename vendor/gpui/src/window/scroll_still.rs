//! KaminIDE patch: retained layers a scrolled replay draws as they were.
//!
//! A part's commands may draw a retained layer (a viewport-fixed box lifted
//! above the flow): the copy draws the layer's scene as it was recorded.
//! That is the step's paint only when all the layer holds stays put: every
//! command in it is marked under a node the plan replays there unmoved,
//! with its clip unchanged (`plan_scrolled_replay`, an anchor cancelling its
//! parent's move), and it holds no nested layer, bottom layer or depth sort.

use crate::ScrollNodeId;
use crate::scene::PaintOperation;
use crate::scene::scroll_marks::SceneSpace;

/// The scenes of the retained layers drawn in `spans` whose every command
/// lies under one of `still` (nodes replayed unmoved in such a layer).
pub(super) fn still_layers<'a>(still: &[ScrollNodeId], spans: impl Iterator<Item = &'a [PaintOperation]>) -> Vec<SceneSpace> {
    if still.is_empty() {
        return Vec::new();
    }
    let stays = |id: ScrollNodeId| still.contains(&id);
    let mut layers = Vec::new();
    for operation in spans.flatten() {
        let PaintOperation::Layer(op) = operation else { continue };
        let Some(cell) = op.cell.upgrade() else { continue };
        let content = cell.content.borrow();
        let scene = content.scene();
        let all = 0..scene.paint_operations.len();
        let reservoirs = scene.paint_operations.iter().all(|operation| match operation {
            PaintOperation::Reservoir(reservoir) => stays(reservoir.run.node),
            _ => true,
        });
        if reservoirs && scene.scroll.marks.iter().all(|mark| stays(mark.node)) && scene.scrollable(&all, std::iter::once(all.clone())) {
            layers.push(scene.scroll.space);
        }
    }
    layers
}
