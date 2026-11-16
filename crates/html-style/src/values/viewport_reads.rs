//! Whether a computation read the size of the viewport (`track`).
//!
//! The computed stage folds viewport-relative units (`vw`, `vh`, `vi`, `vb`,
//! `vmin`, `vmax`, and container units with no query container) into pixels.
//! Each such fold notes the read in the document's values (`note`), so a
//! materialized style carries the viewport among its inputs only when its
//! values depend on it. A new viewport width then recomputes those elements
//! alone. Blink marks the same fact on the style during length conversion
//! (`CSSToLengthConversionData::Flags`, `ComputedStyle::HasViewportUnits`) and
//! restyles only those elements on resize (`StyleEngine::ViewportUnitsChanged`).

use super::{DocumentValues, ValueCx};

/// A fold read the viewport size.
pub fn note(vcx: ValueCx<'_>) {
    vcx.doc.traversal.viewport_read.set(true);
}

/// Run `f` and tell whether it read the viewport size of `doc`. A read inside
/// also counts for an enclosing `track`.
pub fn track<R>(doc: &DocumentValues, f: impl FnOnce() -> R) -> (R, bool) {
    let outer = doc.traversal.viewport_read.replace(false);
    let out = f();
    let inner = doc.traversal.viewport_read.get();
    doc.traversal.viewport_read.set(outer || inner);
    (out, inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_counts_for_its_track_and_every_enclosing_one() {
        let doc = DocumentValues::detached(crate::style::css::Media::default());
        let vcx = doc.values();
        let ((), outer) = track(&doc, || {
            let ((), none) = track(&doc, || ());
            assert!(!none);
            let ((), inner) = track(&doc, || note(vcx));
            assert!(inner);
        });
        assert!(outer);
        let ((), after) = track(&doc, || ());
        assert!(!after);
    }
}
