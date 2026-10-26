//! CSS-привязка заливки по точной геометрии бокса и преобразования.

use gpui::{Bounds, Pixels, Style, Window, point, px};

pub(crate) fn bounds(style: &Style, bounds: Bounds<Pixels>, window: &Window) -> Bounds<Pixels> {
    bounds_with_exact(style, bounds, window.raster_frame().exact_bounds, window)
}

pub(super) fn bounds_with_exact(
    style: &Style,
    bounds: Bounds<Pixels>,
    exact_bounds: Option<(Bounds<Pixels>, Bounds<Pixels>)>,
    window: &Window,
) -> Bounds<Pixels> {
    if !style.snap_to_device_pixels
        || style.border_widths.any(|l| !l.is_zero())
        || style.corner_radii.to_pixels(window.rem_size()).max() != Pixels::ZERO
    {
        return bounds;
    }
    let paint = window.current_transformation();
    let exact_transform = window
        .raster_frame()
        .fill_transform
        .filter(|(current, _)| *current == paint);
    let m = exact_transform.map_or(paint, |(_, exact)| exact);
    let [[a, b], [c, d]] = m.rotation_scale;
    let diagonal = b.abs() <= 1e-5
        && c.abs() <= 1e-5
        && a.abs() > 1e-5
        && d.abs() > 1e-5
        && ((a - 1.0).abs() >= 1e-5 || (d - 1.0).abs() >= 1e-5 || m.translation == [0.0, 0.0]);
    // Заливка привязывается по конечным границам после размещения бокса.
    // Диагональные отражения и знаковые перестановки осей сохраняют
    // прямоугольник: округляем оба преобразованных ребра.
    if !diagonal && exact_transform.is_none() {
        return bounds;
    }
    let det = a * d - b * c;
    if !det.is_finite() || det.abs() <= 1e-5 {
        return bounds;
    }
    let Some((_, exact)) = exact_bounds.filter(|(snapped, _)| *snapped == bounds) else {
        return bounds;
    };
    // CSS Transforms 1 §3: reference box преобразуется до растеризации.
    // После привязки нужных рёбер обращаем фактическую матрицу renderer.
    let sf = window.scale_factor();
    let [[pa, pb], [pc, pd]] = paint.rotation_scale;
    let pdet = pa * pd - pb * pc;
    let edge = |x: Pixels, y: Pixels| {
        let (x, y) = (f32::from(x) * sf, f32::from(y) * sf);
        // Сетка 1/64 с усечением удерживает точную половину пикселя
        // при шуме f32 в цепочке transforms. Guard 0.05 поглощает этот шум,
        // сохраняя 53.498 ниже половины; порядок операций остаётся общим
        // с четверть-оборотами текста и Blink LayoutUnit(float).
        let snap = |v: f32| ((v * 64.0 + 0.05).floor() / 64.0 + 0.5).floor();
        let dx = snap(a * x + b * y + m.translation[0]) - paint.translation[0];
        let dy = snap(c * x + d * y + m.translation[1]) - paint.translation[1];
        point(
            px((pd * dx - pb * dy) / (pdet * sf)),
            px((-pc * dx + pa * dy) / (pdet * sf)),
        )
    };
    Bounds::from_corners(
        edge(exact.left(), exact.top()),
        edge(exact.right(), exact.bottom()),
    )
}
