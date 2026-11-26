//! Плоская матрица преобразования в точках устройства: точка отсчёта и проценты — от
//! опорной коробки (своей, ряда таблицы `ref_box` или фрагмента, css-break-3 §5.5);
//! поворот на четверть ставит края на сетку, как у той же коробки на месте.

use crate::paint::effects::transform_geometry::quarter_turn;
use crate::paint::effects::transformed_element::Transformed;
use gpui::{Bounds, Pixels, Point, QuarterSnap, Size, TransformPivot, TransformationMatrix};
use std::rc::Rc;

/// Опорная коробка: ширина, высота и смещение от начала коробки (`raw_origin`).
pub(super) type Reference = (f32, f32, f32, f32);

pub(super) fn reference(
    group: &Transformed,
    raw_origin: Point<Pixels>,
    exact: Size<Pixels>,
    fragment: Option<Bounds<Pixels>>,
) -> Reference {
    let (w, h) = (f32::from(exact.width), f32::from(exact.height));
    // Reference box of a table row/row group spread over its cells
    // (`ref_box`): origin and percentages resolve against it, offset
    // from this cell's own box.
    match group.ref_box.as_ref().and_then(|r| r.get()) {
        Some(r) => (
            f32::from(r.size.width),
            f32::from(r.size.height),
            f32::from(r.origin.x - raw_origin.x),
            f32::from(r.origin.y - raw_origin.y),
        ),
        None => fragment.map_or((w, h, 0.0, 0.0), |r| {
            (
                f32::from(r.size.width),
                f32::from(r.size.height),
                f32::from(r.origin.x - raw_origin.x),
                f32::from(r.origin.y - raw_origin.y),
            )
        }),
    }
}

/// Точка отсчёта в опорной коробке.
pub(super) fn origin(group: &Transformed, (rw, rh, rdx, rdy): Reference) -> (f32, f32) {
    let ox = rdx + rw * group.origin.0 + group.origin_px.0.unwrap_or(0.0);
    let oy = rdy + rh * group.origin.1 + group.origin_px.1.unwrap_or(0.0);
    (ox, oy)
}

/// The flat path's matrix, the exact fill matrix of a quarter turn, and the
/// matrix inputs (`gpui::TransformPivot`) when no transform is outside or
/// the outer one has its own (`outer`): a scrolled replay builds the same
/// matrix for the moved box from them by the same function. `unit`: no
/// transform applies outside.
pub(super) fn matrix(
    group: &Transformed,
    (bounds, raw_origin, scaled_origin): (Bounds<Pixels>, Point<Pixels>, Point<Pixels>),
    exact: Size<Pixels>,
    r: Reference,
    (scale_factor, unit): (f32, bool),
    outer: Option<Rc<TransformPivot>>,
) -> (
    TransformationMatrix,
    Option<TransformationMatrix>,
    Option<TransformPivot>,
) {
    let (rw, rh) = (r.0, r.1);
    let (ox, oy) = origin(group, r);
    // Матрица функций в порядке записи (css-transforms-1
    // §transform-rendering), вокруг точки отсчёта: она уводится в ноль и
    // возвращается. Проценты сдвига считаются от собственного размера —
    // он известен только здесь, на отрисовке.
    let shift = |row: [f32; 3]| (row[0] + rw * row[1] + rh * row[2]) * scale_factor;
    // A quarter turn (or reflection) keeps the box axis-aligned: its edges
    // round to the device grid as the same box laid out in place does
    // (`gpui::QuarterSnap`; `offset-path-ray-011/013/014` against their
    // `translate(...)` references).
    let quarter = quarter_turn(group.lin).filter(|_| unit);
    let snap = quarter.map(|_| QuarterSnap {
        bounds,
        exact_origin: group.exact_origin.unwrap_or(bounds.origin),
        exact_size: [f32::from(exact.width), f32::from(exact.height)],
        raw_origin,
    });
    // Under an outer transform the inputs exist only when it has its own.
    let outer = outer.filter(|_| !unit);
    let pivoted = unit || outer.is_some();
    let pivot = TransformPivot::new(
        quarter.unwrap_or(group.lin),
        [shift(group.tr[0]), shift(group.tr[1])],
        (scaled_origin, [ox, oy]),
        scale_factor,
        (snap, outer),
    );
    (pivot.local(), pivot.fill(), pivoted.then_some(pivot))
}
