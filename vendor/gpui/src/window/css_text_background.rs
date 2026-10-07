//! Retain an opaque CSS fill's device frame for text painted by its descendants.

use super::*;
use crate::Fill;

impl Window {
    pub(crate) fn with_css_text_background<R>(
        &mut self,
        style: &Style,
        bounds: Bounds<Pixels>,
        paint: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let depth = self.css_text_backgrounds.len();
        // Host/UI fills do not establish a CSS text frame. Rounded and
        // translucent fills need their own coverage, so keep the existing
        // local paragraph policy for those paths.
        let opaque = style
            .background
            .as_ref()
            .and_then(Fill::color)
            .is_some_and(|fill| {
                fill.tag == crate::color::BackgroundTag::Solid && fill.solid.a == 1.0
            });
        if style.css_border_snap
            && !style.css_synthetic_background
            && opaque
            && self.element_opacity() == 1.0
            && style.corner_radii == Default::default()
            && bounds.size.width > px(0.0)
            && bounds.size.height > px(0.0)
        {
            let exact = self.css_exact_bounds.map_or(bounds, |(_, exact)| exact);
            self.css_text_backgrounds.push((
                exact,
                exact.origin - bounds.origin,
                self.current_transformation(),
            ));
        }
        let result = paint(self);
        self.css_text_backgrounds.truncate(depth);
        result
    }

    /// A background does not inherit as a CSS property, but a transparent
    /// child sees it. Use the containing fill's frame only in the same paint
    /// coordinates; positioned text outside it keeps its layout origin.
    pub fn css_text_background_offset(&self, baseline: Point<Pixels>) -> Option<Point<Pixels>> {
        let transform = self.current_transformation();
        self.css_text_backgrounds
            .iter()
            .rev()
            .find_map(|(bounds, offset, frame)| {
                (*frame == transform && bounds.contains(&baseline)).then_some(*offset)
            })
    }
}
