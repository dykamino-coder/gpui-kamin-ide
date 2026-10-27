//! Per-document frame state of the layout phase: anchor registries, the prepaint transform
//! stack and remembered `position-try` choices (`AnchorState`), and the root's measured left
//! edge under `vertical-rl` (`root_left`). The document owns one shared instance and hands it
//! to its builds through `LayoutCx::layout`; an embedded document has its own. Elements that
//! read or write it at prepaint or paint retain this exact `Rc`, not the value context.

use crate::layout::positioned::anchor::AnchorState;
use std::cell::Cell;

#[derive(Default)]
pub(crate) struct LayoutState {
    /// Anchor geometry and fallback selection belonging to this document.
    pub(crate) anchors: AnchorState,
    /// Left edge of the root's content-sized box under `vertical-rl`
    /// (CSS 2.2 §14.2: the canvas background is positioned from the root
    /// box). The body writes it at prepaint, the canvas layer reads it at
    /// paint of the same frame; the previous frame's value is the fallback.
    pub(crate) root_left: Cell<Option<f32>>,
}
