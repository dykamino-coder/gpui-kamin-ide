//! Physical float opportunities and direction-dependent block origins.

use super::EPS;

pub(super) struct Edges {
    start: f32,
    end: f32,
    has_left: bool,
    has_right: bool,
}

impl Edges {
    pub(super) fn new(
        left: f32,
        right: f32,
        root: (f32, f32),
        containing: (f32, f32),
        margins: (f32, f32),
    ) -> Self {
        let (ml, mr) = margins;
        let has_left = left > root.0 + EPS;
        let has_right = right < root.1 - EPS;
        let (start, end) = if !has_left && !has_right {
            (left + ml, right - mr)
        } else {
            (
                left.max(containing.0 + ml.max(0.0)),
                right.min(containing.1 - mr.max(0.0)),
            )
        };
        Self {
            start,
            end,
            has_left,
            has_right,
        }
    }

    pub(super) fn available(&self) -> f32 {
        (self.end - self.start).max(0.0)
    }

    /// CSS 2.1 §10.3.3 preserves the containing block's start-side margin
    /// when the width equation is over-constrained. Keep the unclamped end
    /// so an RTL zero-width block can still overlap a left float and defer.
    pub(super) fn origin(&self, width: f32, rtl: bool) -> f32 {
        if rtl { self.end - width } else { self.start }
    }

    pub(super) fn fits(&self, left: f32, right: f32, x: f32, width: f32) -> bool {
        !(self.has_left && x < left - EPS)
            && !(self.has_right && x + width > right + EPS)
            && !((self.has_left || self.has_right) && width > right - left + EPS)
    }
}
