//! Контур внутренней коробки с сохранением центров углов.

use super::geometry::Radii;
use crate::paint::background::contour::{Seg, Shaped, outer};
use gpui::{Bounds, Pixels};
use kurbo::{BezPath, ParamCurve, Point};

pub(super) fn contour(bounds: Bounds<Pixels>, radii: Radii) -> BezPath {
    let shape = outer(&Shaped {
        rect: (
            bounds.left().into(),
            bounds.top().into(),
            bounds.size.width.into(),
            bounds.size.height.into(),
        ),
        radii,
        k: [1.0; 4],
    });
    let p = |(x, y): (f32, f32)| (f64::from(x), f64::from(y));
    let mut path = BezPath::new();
    path.move_to(p(shape.start));
    for seg in shape.segs {
        match seg {
            Seg::Line(to) => path.line_to(p(to)),
            Seg::Cubic(a, b, to) => path.curve_to(p(a), p(b), p(to)),
        }
    }
    path.close_path();
    // Внутренние радиусы не нормализуются повторно: иначе сместятся центры.
    // Пересекаем контур с его коробкой, сохраняя оставшиеся участки кривых.
    let [left, top, right, bottom] = [bounds.left(), bounds.top(), bounds.right(), bounds.bottom()]
        .map(|v| f64::from(f32::from(v)));
    for (axis, edge, sign) in [
        (0, left, 1.0),
        (0, right, -1.0),
        (1, top, 1.0),
        (1, bottom, -1.0),
    ] {
        path = half_plane(&path, |p| {
            (if axis == 0 { p.x - edge } else { p.y - edge }) * sign
        });
    }
    path
}

fn half_plane(path: &BezPath, distance: impl Fn(Point) -> f64) -> BezPath {
    let mut clipped = BezPath::new();
    for segment in path.segments() {
        let (a, b) = (distance(segment.eval(0.0)), distance(segment.eval(1.0)));
        if a < 0.0 && b < 0.0 {
            continue;
        }
        let segment = if (a < 0.0) != (b < 0.0) {
            // Все четверти эллипса и соединяющие отрезки монотонны по x/y.
            // Деление de Casteljau сохраняет кривую и её касательную.
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..48 {
                let mid = (lo + hi) * 0.5;
                if (distance(segment.eval(mid)) < 0.0) == (a < 0.0) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let t = (lo + hi) * 0.5;
            segment.subsegment(if a < 0.0 { t..1.0 } else { 0.0..t })
        } else {
            segment
        };
        let start = segment.eval(0.0);
        if clipped.is_empty() {
            clipped.move_to(start);
        } else {
            clipped.line_to(start);
        }
        clipped.push(segment.as_path_el());
    }
    if !clipped.is_empty() {
        clipped.close_path();
    }
    clipped
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{ParamCurve, Shape};

    #[test]
    fn asymmetric_inset_stays_in_its_rectangle_without_moving_the_ellipse() {
        let outer = Bounds::new(
            gpui::point(gpui::px(0.0), gpui::px(0.0)),
            gpui::size(gpui::px(100.0), gpui::px(100.0)),
        );
        let (bounds, radii) =
            super::super::geometry::inset(outer, [(50.0, 50.0); 4], [0.0, 80.0, 0.0, 0.0]);
        let path = contour(bounds, radii);
        let rect = kurbo::Rect::new(0.0, 0.0, 20.0, 100.0);
        for seg in path.segments() {
            for i in 0..=100 {
                let p = seg.eval(i as f64 / 100.0);
                assert!(
                    p.x >= rect.x0 - 1e-5
                        && p.x <= rect.x1 + 1e-5
                        && p.y >= rect.y0 - 1e-5
                        && p.y <= rect.y1 + 1e-5,
                    "{p:?}"
                );
            }
        }
        assert_eq!(path.winding((10.0, 50.0).into()), 1);
        assert_eq!(path.winding((10.0, 5.0).into()), 0);
        assert_eq!(path.winding((25.0, 50.0).into()), 0);
    }

    #[test]
    fn clipped_corners_match_the_inner_ellipses_on_every_side() {
        let bounds = Bounds::new(
            gpui::point(gpui::px(0.0), gpui::px(0.0)),
            gpui::size(gpui::px(100.0), gpui::px(100.0)),
        );
        for cuts in [
            [0.0, 80.0, 0.0, 0.0],
            [80.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 80.0, 0.0],
            [0.0, 0.0, 0.0, 80.0],
            [3.0, 71.0, 9.0, 2.0],
            [80.0, 80.0, 0.0, 0.0],
            [0.0, 0.0, 80.0, 80.0],
            [17.0, 71.0, 59.0, 2.0],
            [69.0, 3.0, 7.0, 81.0],
            [13.0, 63.0, 51.0, 29.0],
        ] {
            let (rect, radii) = super::super::geometry::inset(
                bounds,
                [(50.0, 50.0), (40.0, 30.0), (30.0, 40.0), (50.0, 50.0)],
                cuts,
            );
            let path = contour(rect, radii);
            let [x0, y0, x1, y1] =
                [rect.left(), rect.top(), rect.right(), rect.bottom()].map(f32::from);
            for y in 0..100 {
                for x in 0..100 {
                    let (x, y) = (x as f32 + 0.31, y as f32 + 0.73);
                    let mut inside = x >= x0 && x <= x1 && y >= y0 && y <= y1;
                    let mut near_curve = false;
                    for (i, (sx, sy)) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                        .into_iter()
                        .enumerate()
                    {
                        let (rx, ry) = radii[i];
                        let cx = if sx < 0.0 { x0 + rx } else { x1 - rx };
                        let cy = if sy < 0.0 { y0 + ry } else { y1 - ry };
                        if rx > 0.0 && ry > 0.0 && (x - cx) * sx > 0.0 && (y - cy) * sy > 0.0 {
                            let q = ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2);
                            near_curve |= (q - 1.0).abs() < 0.003;
                            inside &= q <= 1.0;
                        }
                    }
                    if !near_curve {
                        assert_eq!(
                            path.winding((f64::from(x), f64::from(y)).into()) != 0,
                            inside,
                            "cuts={cuts:?}, ({x}, {y})"
                        );
                    }
                }
            }
        }
    }
}
