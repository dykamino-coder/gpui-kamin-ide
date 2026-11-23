//! The painting area of a background layer, shared by prepaint and paint.

use super::tiles::LayerPaint;
use crate::style::computed::{Computed, Sides};
use crate::style::values::value::Len;
use gpui::{Bounds, Pixels, px};

pub(super) fn resolve(
    paint: &LayerPaint<'_>,
    c: &Computed,
    paint_base: Bounds<Pixels>,
    border: &Sides,
    px_of: impl Fn(Option<Len>) -> f32,
) -> Bounds<Pixels> {
    match paint.layer.clip {
        Some(crate::style::computed::BgClip::PaddingBox)
        | Some(crate::style::computed::BgClip::Text) => paint_base,
        Some(crate::style::computed::BgClip::ContentBox) => Bounds {
            origin: gpui::point(
                paint_base.origin.x + px(px_of(c.reset_box_layout.padding.left.clone())),
                paint_base.origin.y + px(px_of(c.reset_box_layout.padding.top.clone())),
            ),
            size: gpui::size(
                (paint_base.size.width
                    - px(px_of(c.reset_box_layout.padding.left.clone())
                        + px_of(c.reset_box_layout.padding.right.clone())))
                .max(px(0.0)),
                (paint_base.size.height
                    - px(px_of(c.reset_box_layout.padding.top.clone())
                        + px_of(c.reset_box_layout.padding.bottom.clone())))
                .max(px(0.0)),
            ),
        },
        _ => {
            let covers = |i: usize| {
                let opaque = paint.opaque_borders[i];
                matches!(c.reset_border.border_side_styles[i], Some(3..=6) | Some(9)) && opaque
            };
            let ext = |side: Option<Len>, i: usize| if covers(i) { 0.0 } else { px_of(side) };
            let (t, r, b, l) = (
                ext(border.top.clone(), 0),
                ext(border.right.clone(), 1),
                ext(border.bottom.clone(), 2),
                ext(border.left.clone(), 3),
            );
            Bounds {
                origin: gpui::point(paint_base.origin.x - px(l), paint_base.origin.y - px(t)),
                size: gpui::size(
                    paint_base.size.width + px(l + r),
                    paint_base.size.height + px(t + b),
                ),
            }
        }
    }
}
