//! What the viewport of a document establishes for every box in it: the
//! scrollbars that viewport units exclude and the gutters that shrink the
//! initial containing block. These are facts of the document's view (Blink
//! keeps them on `LocalFrameView`), not CSS values of a box, so they live
//! here once instead of being propagated into the computed style of each box.

use std::cell::Cell;

/// The viewport facts of one document.
#[derive(Default)]
pub struct ViewportFacts {
    unit_insets: Cell<[f32; 2]>,
    icb_scroll_insets: Cell<[f32; 4]>,
}

impl ViewportFacts {
    /// Scrollbar sizes subtracted from the viewport for `vw`/`vh` (css-values-4
    /// §6.1.2; Blink `ComputeScrollbarWidthsForViewportUnits`): set when the
    /// document is prepared from its root's overflow policy.
    pub fn unit_insets(&self) -> [f32; 2] {
        self.unit_insets.get()
    }

    pub fn set_unit_insets(&self, insets: [f32; 2]) {
        self.unit_insets.set(insets);
    }

    /// The viewport's used scrollbar gutters, left/top/right/bottom, that
    /// reduce the initial containing block of absolute and fixed boxes
    /// (css-overflow-3 §4.2): set when the frame builds the viewport box.
    pub fn icb_scroll_insets(&self) -> [f32; 4] {
        self.icb_scroll_insets.get()
    }

    pub fn set_icb_scroll_insets(&self, insets: [f32; 4]) {
        self.icb_scroll_insets.set(insets);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_document_starts_without_viewport_insets_and_keeps_the_last_set() {
        let doc = crate::values::DocumentValues::detached(Default::default());
        assert_eq!(doc.viewport.unit_insets(), [0.0; 2]);
        assert_eq!(doc.viewport.icb_scroll_insets(), [0.0; 4]);
        doc.viewport.set_unit_insets([15.0, 0.0]);
        doc.viewport.set_icb_scroll_insets([0.0, 0.0, 15.0, 0.0]);
        assert_eq!(doc.viewport.unit_insets(), [15.0, 0.0]);
        assert_eq!(doc.viewport.icb_scroll_insets(), [0.0, 0.0, 15.0, 0.0]);
    }
}
