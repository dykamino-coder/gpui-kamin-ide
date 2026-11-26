//! Плоская матрица преобразования в точках устройства: точка отсчёта и проценты — от
//! опорной коробки (своей, ряда таблицы `ref_box` или фрагмента, css-break-3 §5.5);
//! поворот на четверть ставит края на сетку, как у той же коробки на месте.

use crate::paint::effects::transform_geometry::quarter_turn;
use crate::paint::effects::transformed_element::Transformed;
use gpui::{Bounds, Pixels, Point, QuarterSnap, Size, TransformPivot, TransformationMatrix};

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

/// Матрица плоского пути; второе — точная матрица заливки поворота на четверть;
/// третье — входы матрицы (`gpui::TransformPivot`), когда снаружи нет
/// преобразования: повтор прокрутки строит из них ту же матрицу у сдвинутой
/// коробки той же функцией. `unit` — снаружи нет действующего преобразования.
pub(super) fn matrix(
    group: &Transformed,
    (bounds, raw_origin, scaled_origin): (Bounds<Pixels>, Point<Pixels>, Point<Pixels>),
    exact: Size<Pixels>,
    r: Reference,
    (scale_factor, unit): (f32, bool),
) -> (TransformationMatrix, Option<TransformationMatrix>, Option<TransformPivot>) {
    let (rw, rh) = (r.0, r.1);
    let (ox, oy) = origin(group, r);
    // Матрица функций в порядке записи (css-transforms-1
    // §transform-rendering), вокруг точки отсчёта: она уводится в ноль и
    // возвращается. Проценты сдвига считаются от собственного размера —
    // он известен только здесь, на отрисовке.
    let shift = |row: [f32; 3]| (row[0] + rw * row[1] + rh * row[2]) * scale_factor;
    // Поворот на кратное четверти (и отражение) оставляет коробку осевой:
    // её края округляются к точке устройства так же, как у той же коробки,
    // разложенной на месте (`gpui::QuarterSnap`; `offset-path-ray-011/013/014`
    // против эталона `translate(...)`).
    let quarter = quarter_turn(group.lin).filter(|_| unit);
    let snap = quarter.map(|_| QuarterSnap {
        bounds,
        exact_origin: group.exact_origin.unwrap_or(bounds.origin),
        exact_size: [f32::from(exact.width), f32::from(exact.height)],
        raw_origin,
    });
    let pivot = TransformPivot::new(
        quarter.unwrap_or(group.lin),
        [shift(group.tr[0]), shift(group.tr[1])],
        (scaled_origin, [ox, oy]),
        scale_factor,
        snap,
    );
    (pivot.local(), pivot.fill(), unit.then_some(pivot))
}