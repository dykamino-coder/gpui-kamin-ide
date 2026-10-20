//! Paint an oversized circular corner without the quad's half-side radius clamp.
//! CSS Backgrounds 3 sections 4.5 and 4.3 constrain adjacent sums, then inset
//! the same curve for padding/content clips. A clipped larger circle retains
//! the native quad coverage used by ordinary circular backgrounds.

mod geometry;
use crate::hooks::IntoHooked;
use crate::style::computed::{BgClip, Computed};
use crate::style::values::value::{Color, Len};
pub(crate) use geometry::eligible;
use geometry::{inset, square, uniform, virtual_circle};
use gpui::{AnyElement, Bounds, Pixels, Window, px};

fn paint_fill(window: &mut Window, rect: Bounds<Pixels>, i: usize, r: f32, color: Color) {
    let circle = super::sampling::snapped_clip(virtual_circle(rect, i, r), window);
    paint_area(window, rect, i, r, color, circle);
}

fn paint_area(
    window: &mut Window,
    rect: Bounds<Pixels>,
    i: usize,
    r: f32,
    color: Color,
    circle: Bounds<Pixels>,
) {
    if color.a == 0.0 || rect.size.width <= px(0.0) || rect.size.height <= px(0.0) {
        return;
    }
    let fill = color.to_hsla();
    if r <= 0.0 {
        window.paint_quad(gpui::fill(rect, fill));
        return;
    }
    let part = square(rect, i, r);
    crate::paint::decorations::rect_minus::paint_rect_minus(
        window,
        (
            rect.left().into(),
            rect.top().into(),
            rect.right().into(),
            rect.bottom().into(),
        ),
        (
            part.left().into(),
            part.top().into(),
            part.right().into(),
            part.bottom().into(),
        ),
        fill,
    );
    let radius = circle.size.width.min(circle.size.height) * 0.5;
    window.with_content_mask(Some(gpui::ContentMask { bounds: rect }), |window| {
        window.with_content_mask(Some(gpui::ContentMask { bounds: part }), |window| {
            window.paint_quad(gpui::fill(circle, fill).corner_radii(radius));
        });
    });
}

pub(crate) fn layer(c: &Computed) -> Option<AnyElement> {
    if !eligible(c) {
        return None;
    }
    let (i, radius) = [c.radius.tl, c.radius.tr, c.radius.br, c.radius.bl]
        .into_iter()
        .enumerate()
        .find_map(|(i, r)| match r {
            Some(r @ Len::Px(v)) | Some(r @ Len::Pct(v)) if v > 0.0 => Some((i, r)),
            _ => None,
        })?;
    let b = uniform(c.borders())?;
    let p = uniform(c.padding)?;
    let background = c.background.unwrap_or_default();
    let border = c
        .border_colors
        .iter()
        .flatten()
        .next()
        .copied()
        .or(c.border_color)
        .or(c.color)
        .unwrap_or(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
    let clip = c.bg_clip.unwrap_or(BgClip::BorderBox);
    Some(
        super::exact_layer::ExactLayer::inset([-b; 4], move |geometry, window| {
            let bounds = geometry.positioning(geometry.snapped);
            let size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let r = match radius {
                Len::Px(v) => v,
                Len::Pct(p) => p * size.0,
                _ => return,
            };
            let r = r.min(size.0).min(size.1);
            // An opaque same-color background and border have no internal seam.
            if border == background && border.a == 1.0 && (clip != BgClip::ContentBox || p == 0.0) {
                paint_fill(window, bounds, i, r, background);
                return;
            }
            let n = match clip {
                BgClip::PaddingBox => b,
                BgClip::ContentBox => b + p,
                _ => 0.0,
            };
            let joined = b > 0.0 && border.a == 1.0 && (clip != BgClip::ContentBox || p == 0.0);
            if !joined {
                // CSS corner-shaping retains the outer curve's center.
                // Apply the complete inset once to its device geometry;
                // independently snapping another circle moves its center.
                let circle = super::sampling::snapped_clip(virtual_circle(bounds, i, r), window);
                let used = (n * window.scale_factor()).floor() / window.scale_factor();
                paint_area(
                    window,
                    inset(bounds, n),
                    i,
                    (r - n).max(0.0),
                    background,
                    inset(circle, used),
                );
            }
            if b <= 0.0 || border.a == 0.0 {
                return;
            }
            let padding = inset(bounds, b);
            let rects = [
                Bounds::from_corners(bounds.origin, gpui::point(bounds.right(), padding.top())),
                Bounds::from_corners(
                    gpui::point(bounds.left(), padding.bottom()),
                    gpui::point(bounds.right(), bounds.bottom()),
                ),
                Bounds::from_corners(
                    gpui::point(bounds.left(), padding.top()),
                    gpui::point(padding.left(), padding.bottom()),
                ),
                Bounds::from_corners(
                    gpui::point(padding.right(), padding.top()),
                    gpui::point(bounds.right(), padding.bottom()),
                ),
            ];
            for rect in rects {
                window.with_content_mask(Some(gpui::ContentMask { bounds: rect }), |window| {
                    paint_fill(window, bounds, i, r, border)
                });
            }
            let circle = super::sampling::snapped_clip(virtual_circle(bounds, i, r), window);
            let radius = circle.size.width.min(circle.size.height) * 0.5;
            let width = px((b * window.scale_factor()).floor() / window.scale_factor());
            // Native coverage combines the two colors before compositing.
            // Painting black under a white AA border in separate passes
            // leaves black coverage at the outer edge of the white border.
            let fill = if joined {
                background.to_hsla()
            } else {
                gpui::transparent_black()
            };
            let ring = gpui::fill(circle, fill)
                .corner_radii(radius)
                .border_widths(width)
                .border_color(border.to_hsla());
            window.with_content_mask(Some(gpui::ContentMask { bounds: padding }), |window| {
                if joined {
                    let part = square(bounds, i, r);
                    crate::paint::decorations::rect_minus::paint_rect_minus(
                        window,
                        (
                            padding.left().into(),
                            padding.top().into(),
                            padding.right().into(),
                            padding.bottom().into(),
                        ),
                        (
                            part.left().into(),
                            part.top().into(),
                            part.right().into(),
                            part.bottom().into(),
                        ),
                        background.to_hsla(),
                    );
                }
                window.with_content_mask(
                    Some(gpui::ContentMask {
                        bounds: square(bounds, i, r),
                    }),
                    |window| window.paint_quad(ring),
                );
            });
        })
        .hooked(),
    )
}
