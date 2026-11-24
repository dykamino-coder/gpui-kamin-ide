//! KaminIDE patch: the clip of a laid out node's ancestors, known before prepaint.

use super::Window;
use crate::{Bounds, ContentMask, LayoutId, Pixels};

impl Window {
    /// The content mask node `id` of the current engine prepaints under when
    /// it is laid out below a root prepainted under `root` and the only clips
    /// between them are its ancestors' overflow clips
    /// (`TaffyLayoutEngine::overflow_clip`), in window coordinates without an
    /// element offset. A host that recorded a subtree whose actual mask was
    /// this one can test a later frame's layout against it before prepaint, as
    /// Blink compares a cached subsequence's property tree state; a clip the
    /// layout does not know (an element's own `with_content_mask`, a clip
    /// margin) makes the actual mask differ, and such a record is not testable.
    pub fn layout_content_mask(
        &mut self,
        id: LayoutId,
        root: ContentMask<Pixels>,
    ) -> ContentMask<Pixels> {
        let scale = self.scale_factor();
        let mut bounds: Bounds<Pixels> = root.bounds;
        let Some(engine) = self.layout_engine.as_mut() else {
            return root;
        };
        let mut at = engine.parent_layout_id(id);
        while let Some(node) = at {
            if let Some(clip) = engine.overflow_clip(node, scale) {
                bounds = clip.intersect(&bounds);
            }
            at = engine.parent_layout_id(node);
        }
        ContentMask { bounds }
    }
}
