//! Координаты линейного градиента независимы от области обрезки (§3.8).

use crate::style::computed::Gradient;
use gpui::{Bounds, Pixels};

pub(super) const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

pub(super) fn rebase(
    gradient: &Gradient,
    positioning: Bounds<Pixels>,
    paint: Bounds<Pixels>,
    matrix: [f32; 6],
) -> Gradient {
    let mut mapped = gradient.clone();
    let angle = gradient.angle_deg.to_radians();
    let (dx, dy) = (angle.sin(), -angle.cos());
    let length = f32::from(positioning.size.width) * dx.abs()
        + f32::from(positioning.size.height) * dy.abs();
    let [a, b, c, d, tx, ty] = matrix;
    let determinant = a * d - b * c;
    if determinant == 0.0 {
        return mapped; // Вырожденное преобразование не имеет площади краски.
    }
    // Градиент — линейная функция координат. Её направление преобразуется
    // обратной транспонированной матрицей, включая неравномерный scale/skew.
    let (qx, qy) = (
        (d * dx - b * dy) / determinant,
        (-c * dx + a * dy) / determinant,
    );
    let norm = qx.hypot(qy);
    let (ux, uy, norm) = if matrix == IDENTITY {
        (dx, dy, 1.0)
    } else {
        (qx / norm, qy / norm, norm)
    };
    let paint_length =
        f32::from(paint.size.width) * ux.abs() + f32::from(paint.size.height) * uy.abs();
    if paint_length <= 0.0 {
        return mapped;
    }
    let center = positioning.center();
    let (x, y) = (f32::from(center.x), f32::from(center.y));
    let paint_center = paint.center();
    let offset = (a * x + c * y + tx - f32::from(paint_center.x)) * ux
        + (b * x + d * y + ty - f32::from(paint_center.y)) * uy;
    if matrix != IDENTITY {
        mapped.angle_deg = ux.atan2(-uy).to_degrees();
    }
    if mapped.stops.is_empty() {
        mapped.stops = vec![(mapped.from, 0.0), (mapped.to, 1.0)];
    }
    for (_, stop) in &mut mapped.stops {
        *stop = 0.5 + (offset + (*stop - 0.5) * length / norm) / paint_length;
    }
    mapped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::background::ColorBox;
    use crate::style::computed::{BgClip, Computed};
    use gpui::{Edges, point, px, size};

    #[test]
    fn tessellated_path_keeps_the_image_coordinate_under_affine_transforms() {
        let positioning = Bounds::new(point(px(10.0), px(20.0)), size(px(100.0), px(80.0)));
        let (clip, radii) =
            super::super::geometry::inset(positioning, [(50.0, 40.0); 4], [0.0, 80.0, 0.0, 0.0]);
        let mut gradient = Gradient::default();
        gradient.angle_deg = 37.0;
        gradient.stops = vec![(gradient.from, -0.2), (gradient.to, 1.3)];
        for matrix in [
            IDENTITY,
            [0.0, 1.0, -1.0, 0.0, 210.0, 7.0],
            [2.0, 0.3, 0.7, 0.6, -9.0, 13.0],
            [-1.0, 0.0, 0.0, 1.0, 200.0, 0.0],
        ] {
            let path = super::super::geometry::contour_path(clip, radii, matrix).unwrap();
            let mapped = rebase(&gradient, positioning, path.bounds, matrix);
            let [a, b, c, d, tx, ty] = matrix;
            let (center, length, _) = axis(positioning, gradient.angle_deg);
            let angle = gradient.angle_deg.to_radians();
            let direction = (angle.sin(), -angle.cos());
            let (paint_center, paint_length, _) = axis(path.bounds, mapped.angle_deg);
            let r = mapped.angle_deg.to_radians();
            for (x, y) in [(15.0, 60.0), (20.0, 70.0), (25.0, 65.0)] {
                let old = 0.5 + (x * direction.0 + y * direction.1 - center) / length;
                let transformed = (a * x + c * y + tx, b * x + d * y + ty);
                let new = 0.5
                    + (transformed.0 * r.sin() - transformed.1 * r.cos() - paint_center)
                        / paint_length;
                let original_mix =
                    (old - gradient.stops[0].1) / (gradient.stops[1].1 - gradient.stops[0].1);
                let mapped_mix =
                    (new - mapped.stops[0].1) / (mapped.stops[1].1 - mapped.stops[0].1);
                assert!((original_mix - mapped_mix).abs() < 1e-5, "{matrix:?}");
            }
        }
    }

    fn axis(bounds: Bounds<Pixels>, angle: f32) -> (f32, f32, f32) {
        let r = angle.to_radians();
        let (dx, dy) = (r.sin(), -r.cos());
        let center = bounds.center();
        (
            f32::from(center.x) * dx + f32::from(center.y) * dy,
            f32::from(bounds.size.width) * dx.abs() + f32::from(bounds.size.height) * dy.abs(),
            angle,
        )
    }

    #[test]
    fn clipping_does_not_resize_or_reposition_the_gradient_image() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let area = ColorBox::Padding {
            bounds: Bounds::new(point(px(10.0), px(20.0)), size(px(100.0), px(80.0))),
            border: Edges::all(px(10.0)),
            padding: Edges {
                top: px(7.0),
                right: px(31.0),
                bottom: px(13.0),
                left: px(3.0),
            },
        };
        for clip in [BgClip::PaddingBox, BgClip::ContentBox] {
            for radius in ["5px", "50% / 35%"] {
                for angle in [0, 37, 90, 210] {
                    let mut c = Computed::default();
                    c.apply_one(
                        "background-image",
                        &format!("linear-gradient({angle}deg, red, blue)"),
                        vcx,
                    );
                    c.apply_one("background-color", "transparent", vcx);
                    c.apply_one("border-radius", radius, vcx);
                    c.bg_clip = Some(clip);
                    assert!(!c.gradient_as_tile());
                    let super::super::fill::Fill::Linear(_, positioning) =
                        super::super::fill::Fill::new(&c, area, vcx).unwrap()
                    else {
                        panic!("transparent bottom color must preserve the gradient image");
                    };
                    assert_eq!(
                        positioning,
                        super::super::geometry::area(&c, area, BgClip::PaddingBox, vcx).0
                    );
                    let paint = super::super::geometry::area(&c, area, clip, vcx).0;
                    let original = c.gradient.as_ref().unwrap();
                    let mapped = rebase(original, positioning, paint, IDENTITY);
                    let (oc, ol, _) = axis(positioning, original.angle_deg);
                    let (pc, pl, _) = axis(paint, mapped.angle_deg);
                    for ((_, old), (_, new)) in original.stops.iter().zip(&mapped.stops) {
                        assert!(
                            (oc + (old - 0.5) * ol - pc - (new - 0.5) * pl).abs() < 1e-4,
                            "{clip:?} {radius} {angle}: {old} -> {new}"
                        );
                    }
                    assert_eq!(original.from, mapped.from);
                    assert_eq!(original.to, mapped.to);
                    for origin in ["border-box", "padding-box", "content-box"] {
                        c.apply_one("background-origin", origin, vcx);
                        assert_eq!(c.gradient_as_tile(), origin != "padding-box");
                    }
                }
            }
        }
    }
}
