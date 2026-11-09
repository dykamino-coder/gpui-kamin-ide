//! The block records over a rebuild of the same source DOM (`layout::reuse::Store::rebase`).

use crate::document::Document;

impl Document {
    /// `next` rebuilds this document's source DOM with the same node ids: it
    /// takes over the block records. Its styles are computed here, under the
    /// root style, viewport and document font family of the last build, as
    /// that build's style pass would (`render::render`): the style pass of the
    /// next frame then only checks their inputs. With both trees computed the
    /// records are compared now (`Store::rebase`), and the previous frame tree
    /// goes with this document instead of living into the next build (Blink
    /// `Document::UpdateStyleAndLayoutTree` and Stylo's traversal likewise
    /// finish the computed styles before layout reads them).
    pub(super) fn hand_over_reuse(&self, next: &mut Document) {
        if let Some((root, viewport, family)) = self.reuse.frame_inputs() {
            let vcx = next.state.frame_values().with_viewport(viewport);
            crate::text::metrics::set_doc_family(&family, vcx);
            crate::style::cascade::materialize::materialize(&next.nodes, root.style_rc(), vcx);
            self.reuse.rebase(&self.nodes, &next.nodes);
        }
        next.reuse = self.reuse.clone();
    }
}
