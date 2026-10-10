//! Keep the CSS fill matrix before glyph-oriented quarter-turn placement snapping.

use crate::{TransformationMatrix, Window};

impl Window {
    /// Paint CSS box fills from the unrounded matrix while other primitives
    /// retain their existing placement. A nested transformation has its own map.
    pub fn with_css_fill_transform<R>(
        &mut self,
        exact: TransformationMatrix,
        paint: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let current = self.current_transformation();
        let previous = self.css_fill_transform.replace((current, exact));
        let result = paint(self);
        self.css_fill_transform = previous;
        result
    }
}
