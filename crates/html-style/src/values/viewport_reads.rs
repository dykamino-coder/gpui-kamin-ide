//! Whether a computation read the size of the viewport (`track`).
//!
//! The computed stage folds viewport-relative units (`vw`, `vh`, `vi`, `vb`,
//! `vmin`, `vmax`, and container units with no query container) into pixels.
//! Each such fold notes the read (`note`), so a materialized style carries the
//! viewport among its inputs only when its values depend on it. A new
//! viewport width then recomputes those elements alone. Blink marks the same
//! fact on the style during length conversion
//! (`CSSToLengthConversionData::Flags`, `ComputedStyle::HasViewportUnits`) and
//! restyles only those elements on resize (`StyleEngine::ViewportUnitsChanged`).

use std::cell::Cell;

thread_local! {
    static READ: Cell<bool> = const { Cell::new(false) };
}

/// A fold read the viewport size.
pub fn note() {
    READ.with(|read| read.set(true));
}

/// Run `f` and tell whether it read the viewport size. A read inside also
/// counts for an enclosing `track`.
pub fn track<R>(f: impl FnOnce() -> R) -> (R, bool) {
    let outer = READ.with(|read| read.replace(false));
    let out = f();
    let inner = READ.with(|read| read.replace(outer));
    if inner {
        note();
    }
    (out, inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_counts_for_its_track_and_every_enclosing_one() {
        let ((), outer) = track(|| {
            let ((), none) = track(|| ());
            assert!(!none);
            let ((), inner) = track(note);
            assert!(inner);
        });
        assert!(outer);
        let ((), after) = track(|| ());
        assert!(!after);
    }
}
