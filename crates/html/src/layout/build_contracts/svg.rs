//! The replaced CSS box owns decorations; the SVG keeps its inheritance context.

use crate::dom::Element;
use crate::resources::images::svg_raster::RootPaint;

#[derive(Clone, Copy)]
pub(crate) struct SvgContent<'a> {
    pub element: &'a Element,
    pub root_paint: RootPaint,
}

impl<'a> SvgContent<'a> {
    pub(crate) fn replaced(element: &'a Element) -> Self {
        Self {
            element,
            root_paint: RootPaint::CssBox {
                opacity: element.style.used().reset_effects.opacity.map(Into::into),
            },
        }
    }
}
