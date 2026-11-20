//! Layers of the initial containing block: the document, a strip block, a
//! nested document and the root scrollport, whose fixed boxes hang on the
//! viewport (`viewport_anchor`).

use super::super::SpotCell;
use super::super::late::LatePlace;
use super::super::oof::OofQueue;
use super::Positioning;
use crate::hooks::IntoHooked;
use gpui::AnyElement;

impl Positioning {
    /// Open an initial-containing-block layer: the document, a strip block or
    /// a nested document.
    pub fn icb_open(&self) {
        self.icb_open_anchored(None);
    }

    /// Open the ICB layer of a root scrollport scrolled by `anchor`.
    pub(crate) fn icb_open_anchored(&self, anchor: Option<gpui::ScrollHandle>) {
        let queue = OofQueue::initial();
        self.pending.borrow_mut().push(queue.pending());
        self.icb.borrow_mut().push(queue);
        self.anchors.borrow_mut().push(anchor);
    }

    /// The root scroll handle of the top ICB layer: a fixed box placed into
    /// it cancels that offset (css-position-3 §fixed-pos: the viewport is
    /// its containing block and it does not move when the document scrolls).
    pub(crate) fn viewport_anchor(&self) -> Option<gpui::ScrollHandle> {
        self.anchors.borrow().last().cloned().flatten()
    }

    /// Take what the top ICB layer collected and close it.
    pub fn icb_close(&self) -> Vec<AnyElement> {
        self.anchors.borrow_mut().pop();
        self.pending.borrow_mut().pop();
        let layer = self.icb.borrow_mut().pop();
        layer.unwrap_or_default().place(icb_place)
    }

    /// Is an ICB layer open?
    ///
    /// A scroll strip builds its subtree in a closure that
    /// `ScrollArea::request_layout` calls AFTER `icb_close()`: candidates can
    /// be hoisted only while the layer still exists.
    pub fn icb_active(&self) -> bool {
        !self.icb.borrow().is_empty()
    }

    /// Hand an element to the top ICB layer. No layer — it comes back.
    pub fn icb_push(&self, spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
        match self.icb.borrow_mut().last_mut() {
            Some(layer) => {
                layer.push(spot, el);
                None
            }
            None => Some(el),
        }
    }
}

/// The placeholder of an ICB layer: WITHOUT a wrapper, unlike `spot_place`.
///
/// Any box around the child would become the containing block of the
/// absolute child for layout (layout has no notion of a positioned ancestor),
/// and the insets would resolve against it instead of the viewport, which is
/// exactly what the layer avoids. `LatePlace` adds no box of its own: it
/// hands over the child's `layout_id`. The exception is a fixed box of the
/// root scrollport (`Spot::viewport_fixed`): its box coincides with the
/// scrollport, so the insets are the same, and it adds nothing to the
/// scroll range (`viewport_anchor::viewport_box`).
pub(in crate::layout::positioned::containing_block) fn icb_place(
    spot: SpotCell,
    child: AnyElement,
) -> AnyElement {
    let now = spot.get();
    let placed = if now.layout_only {
        child
    } else {
        LatePlace { child, spot }.hooked()
    };
    if now.viewport_fixed {
        crate::layout::positioned::viewport_anchor::viewport_box(placed)
    } else {
        placed
    }
}
