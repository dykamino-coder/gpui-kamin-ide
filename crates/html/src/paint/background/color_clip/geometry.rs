//! Геометрия внутреннего фона: сначала нормализация внешних радиусов,
//! затем вычитание рамки и padding отдельно по каждой оси (§5.3, §5.5).

use crate::document::state::ValueCx;
use crate::style::computed::{BgClip, Computed};
use gpui::{Bounds, PathBuilder, Pixels, Window, point, px};

pub(super) type Radii = [(f32, f32); 4];

pub(super) fn area(
    c: &Computed,
    area: super::ColorBox,
    clip: BgClip,
    vcx: ValueCx<'_>,
) -> (Bounds<Pixels>, Radii) {
    let (padding, border, pad) = match area {
        super::ColorBox::Padding {
            bounds,
            border,
            padding,
        } => (bounds, border, padding),
        // CSS 2.1 §17.5.1 / CSS Backgrounds §4.6: внутренние полосы
        // таблицы не имеют padding и скруглённых краёв.
        super::ColorBox::TableBand(bounds) => return (bounds, [(0.0, 0.0); 4]),
    };
    let border = [border.top, border.right, border.bottom, border.left].map(f32::from);
    let pad = [pad.top, pad.right, pad.bottom, pad.left].map(f32::from);
    let [t, r, b, l] = border;
    let outer = Bounds {
        origin: gpui::point(padding.left() - px(l), padding.top() - px(t)),
        size: gpui::size(
            padding.size.width + px(l + r),
            padding.size.height + px(t + b),
        ),
    };
    let spec = super::super::rrect_spec(c, None, vcx);
    let radii = super::super::shape_raster::parse_rrect(
        spec.trim_start_matches("rrect(").trim_end_matches(')'),
        outer.size.width.into(),
        outer.size.height.into(),
        1.0,
    )
    .map(|r| r.corners)
    .unwrap_or([(0.0, 0.0); 4]);
    let cuts = match clip {
        BgClip::BorderBox => [0.0; 4],
        // В схлопнутой модели border представлен половиной кромки внутри
        // layout padding. Для content-box она уже входит в полный отступ.
        BgClip::ContentBox => std::array::from_fn(|i| border[i] + pad[i]),
        _ => c.bg_border_insets.unwrap_or(border),
    };
    inset(outer, radii, cuts)
}

pub(super) fn inset(
    outer: Bounds<Pixels>,
    radii: Radii,
    [t, r, b, l]: [f32; 4],
) -> (Bounds<Pixels>, Radii) {
    let bounds = Bounds {
        origin: gpui::point(outer.left() + px(l), outer.top() + px(t)),
        size: gpui::size(
            (outer.size.width - px(l + r)).max(px(0.0)),
            (outer.size.height - px(t + b)).max(px(0.0)),
        ),
    };
    let cuts = [(l, t), (r, t), (r, b), (l, b)];
    (
        bounds,
        std::array::from_fn(|i| {
            (
                (radii[i].0 - cuts[i].0).max(0.0),
                (radii[i].1 - cuts[i].1).max(0.0),
            )
        }),
    )
}

pub(super) fn paint_contour(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    radii: Radii,
    fill: &super::fill::Fill,
) {
    // paint_path принимает уже преобразованную геометрию, в отличие от
    // paint_quad; матрица окна задана в физических пикселях устройства.
    let matrix = window.current_transformation();
    let [[a, c], [b, d]] = matrix.rotation_scale;
    let [x, y] = matrix.translation;
    let scale = window.scale_factor();
    let matrix = [a, b, c, d, x / scale, y / scale];
    if let Some(path) = contour_path(bounds, radii, matrix) {
        let background = fill.at(path.bounds, matrix);
        window.paint_path(path, background);
    }
}

pub(super) fn contour_path(
    bounds: Bounds<Pixels>,
    radii: Radii,
    matrix: [f32; 6],
) -> Option<gpui::Path<Pixels>> {
    use kurbo::PathEl;
    let contour = super::bounded::contour(bounds, radii);
    let p = |p: kurbo::Point| point(px(p.x as f32), px(p.y as f32));
    let mut path = PathBuilder::fill();
    for segment in contour.elements() {
        match segment {
            PathEl::MoveTo(to) => path.move_to(p(*to)),
            PathEl::LineTo(to) => path.line_to(p(*to)),
            PathEl::QuadTo(a, to) => path.curve_to(p(*to), p(*a)),
            PathEl::CurveTo(a, b, to) => path.cubic_bezier_to(p(*to), p(*a), p(*b)),
            PathEl::ClosePath => path.close(),
        }
    }
    let [a, b, c, d, x, y] = matrix;
    path.transform(gpui::Transform::new(a, b, c, d, x, y));
    path.build().ok()
}

#[cfg(test)]
mod tests;
