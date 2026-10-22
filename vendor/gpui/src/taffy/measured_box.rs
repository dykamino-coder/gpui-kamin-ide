//! Containing-block dimensions and native resolved box edges of a measured leaf.

use super::*;
use taffy::util::ResolveOrZero;

/// Used edges share Taffy's percentage basis, independently of the leaf's own width.
#[derive(Clone, Copy, Debug, Default)]
pub struct MeasuredBox {
    /// Containing block dimensions, before resolving this leaf's preferred size.
    pub parent_size: Size<Option<Pixels>>,
    /// Used padding, resolved against the containing block inline size.
    pub padding: Edges<Pixels>,
    /// Used border widths in logical pixels.
    pub border: Edges<Pixels>,
}

impl MeasuredBox {
    pub(super) fn from_native(
        parent: taffy::Size<Option<f32>>,
        style: &taffy::Style,
        scale: f32,
    ) -> Self {
        let edges = |rect: taffy::Rect<f32>| Edges {
            top: Pixels(rect.top / scale),
            right: Pixels(rect.right / scale),
            bottom: Pixels(rect.bottom / scale),
            left: Pixels(rect.left / scale),
        };
        Self {
            parent_size: size(
                parent.width.map(|v| Pixels(v / scale)),
                parent.height.map(|v| Pixels(v / scale)),
            ),
            padding: edges(
                style
                    .padding
                    .resolve_or_zero(parent.width, &taffy::tree::calc_value),
            ),
            border: edges(
                style
                    .border
                    .resolve_or_zero(parent.width, &taffy::tree::calc_value),
            ),
        }
    }

    /// Border and padding added outside the measured content size by native leaf layout.
    pub fn inset(self) -> Edges<Pixels> {
        Edges {
            top: self.padding.top + self.border.top,
            right: self.padding.right + self.border.right,
            bottom: self.padding.bottom + self.border.bottom,
            left: self.padding.left + self.border.left,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::px;

    #[test]
    fn percentage_padding_uses_containing_width_on_both_axes_and_scales_once() {
        let style = taffy::Style {
            padding: taffy::Rect {
                top: taffy::LengthPercentage::percent(0.1),
                bottom: taffy::LengthPercentage::percent(0.1),
                left: taffy::LengthPercentage::percent(0.1),
                right: taffy::LengthPercentage::percent(0.1),
            },
            ..Default::default()
        };
        let geometry = MeasuredBox::from_native(
            taffy::Size {
                width: Some(600.),
                height: Some(100.),
            },
            &style,
            2.,
        );
        assert_eq!(geometry.parent_size.width, Some(px(300.)));
        assert_eq!(geometry.padding.top, px(30.));
        assert_eq!(geometry.padding.left, px(30.));
        assert_eq!(geometry.padding.bottom, px(30.));
    }
}
