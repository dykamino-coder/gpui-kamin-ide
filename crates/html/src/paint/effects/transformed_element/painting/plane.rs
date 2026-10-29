//! Плоская матрица преобразования в точках устройства: точка отсчёта и проценты — от
//! опорной коробки (своей, ряда таблицы `ref_box` или фрагмента, css-break-3 §5.5);
//! поворот на четверть ставит края на сетку, как у той же коробки на месте.

use crate::paint::effects::transform_geometry::quarter_turn;
use crate::paint::effects::transformed_element::Transformed;
use gpui::{Bounds, Pixels, Point, Size, TransformationMatrix, px};

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

/// Матрица плоского пути; второе — точная матрица заливки поворота на четверть.
/// `unit` — снаружи нет действующего преобразования.
pub(super) fn matrix(
    group: &Transformed,
    (bounds, raw_origin, scaled_origin): (Bounds<Pixels>, Point<Pixels>, Point<Pixels>),
    exact: Size<Pixels>,
    r: Reference,
    (scale_factor, unit): (f32, bool),
) -> (TransformationMatrix, Option<TransformationMatrix>) {
    let (w, h) = (f32::from(exact.width), f32::from(exact.height));
    let (rw, rh) = (r.0, r.1);
    let (ox, oy) = origin(group, r);
    let dev = |v: f32| px(v).scale(scale_factor);
    let origin = gpui::point(
        dev(f32::from(scaled_origin.x) + ox),
        dev(f32::from(scaled_origin.y) + oy),
    );
    let back = gpui::point(
        dev(-(f32::from(scaled_origin.x) + ox)),
        dev(-(f32::from(scaled_origin.y) + oy)),
    );
    // Матрица функций в порядке записи (css-transforms-1
    // §transform-rendering), вокруг точки отсчёта: она уводится в ноль и
    // возвращается. Проценты сдвига считаются от собственного размера —
    // он известен только здесь, на отрисовке.
    let shift = |row: [f32; 3]| (row[0] + rw * row[1] + rh * row[2]) * scale_factor;
    let quarter = quarter_turn(group.lin).filter(|_| unit);
    let mut matrix = TransformationMatrix::unit()
        .translate(origin)
        .compose(TransformationMatrix {
            rotation_scale: quarter.unwrap_or(group.lin),
            translation: [shift(group.tr[0]), shift(group.tr[1])],
        })
        .translate(back);
    let fill_matrix =
        quarter.map(|_| group.exact_fill_matrix(matrix, bounds.origin, raw_origin, scale_factor));
    if quarter.is_some() {
        // Поворот на кратное четверти (и отражение) оставляет коробку
        // осевой: её края обязаны округляться к точке устройства так
        // же, как у той же коробки, разложенной на месте (чистый сдвиг
        // выше идёт через раскладку — round half up). Растеризатор
        // по правилу «верх-лево» относит ровную половину вниз, а
        // ошибка `f32` у `rotate(-90deg)` (cos ≈ −4e-8) решает
        // ничью случайно — `offset-path-ray-011/013/014` против
        // эталона `translate(...)`. Skia так же кладёт осевой
        // прямоугольник по round(x) (`SkScan::FillRect`), а
        // `gfx::SinCosDegrees` даёт точные 0/±1 у кратных 90°.
        let sf = scale_factor;
        let corner_min = |o: gpui::Point<Pixels>, w: f32, h: f32| {
            let mut m = (f32::INFINITY, f32::INFINITY);
            for (dx, dy) in [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)] {
                let p = matrix.apply(gpui::point(
                    px((f32::from(o.x) + dx) * sf),
                    px((f32::from(o.y) + dy) * sf),
                ));
                m = (m.0.min(f32::from(p.x)), m.1.min(f32::from(p.y)));
            }
            m
        };
        let exact_origin = group.exact_origin.unwrap_or(bounds.origin);
        let exact = corner_min(exact_origin, w, h);
        let cur = corner_min(
            bounds.origin,
            f32::from(bounds.size.width),
            f32::from(bounds.size.height),
        );
        let snap = |v: f32| ((v * 64.0).round() / 64.0 + 0.5).floor();
        matrix.translation[0] += snap(exact.0) - cur.0;
        matrix.translation[1] += snap(exact.1) - cur.1;
    }
    (matrix, fill_matrix)
}
