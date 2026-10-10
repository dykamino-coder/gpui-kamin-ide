//! Окраска плоского и объёмного преобразования поддерева.

mod projection;
pub(super) use projection::paint_3d;

use crate::paint::effects::transform_geometry::quarter_turn;
use crate::paint::effects::transformed_element::Transformed;
use gpui::{App, Bounds, GlobalElementId, InspectorElementId, LayoutId, Pixels, Window, px};

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_body(
    group: &mut Transformed,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&InspectorElementId>,
    bounds: Bounds<Pixels>,
    layout_id: &mut LayoutId,
    _prepaint: &mut (),
    window: &mut Window,
    cx: &mut App,
) {
    if group.placed {
        group.child.as_mut().unwrap().paint(window, cx);
        return;
    }
    let scale_factor = window.scale_factor();
    // CSS transform origins precede device-pixel snapping (Transforms 1 §3).
    let raw_origin = group.exact_origin.unwrap_or(bounds.origin);
    let scaled_origin = group.scaled_origin(bounds.origin);
    let dev = |v: f32| px(v).scale(scale_factor);
    // Точка отсчёта — в устройстве, от неё и разворачиваем. Записанная
    // длиной, она сильнее доли: `transform-origin: 0 0` — левый верх, а
    // не центр (доля из длины считается только здесь, где размер известен).
    // Доля × размер ПЛЮС точки: `calc(50% + 10px)` — смесь, и доля у
    // чистых точек равна нулю (css-transforms-1 §5.2).
    // Доли (`transform-origin: 50%`, `translate(100%)`) — от размера
    // раскладки, а не от округлённых к точке устройства краёв: 50px ×
    // 1.25 = 62.5 округлялось до 63, и `translateY(100%)` уезжал на
    // 0.4px от `translateY(50px)` (`transform-percent-*`).
    let exact = window.layout_size_unrounded(*layout_id);
    let (w, h) = (f32::from(exact.width), f32::from(exact.height));
    // Reference box of a table row/row group spread over its cells
    // (`ref_box`): origin and percentages resolve against it, offset
    // from this cell's own box.
    let (rw, rh, rdx, rdy) = match group.ref_box.as_ref().and_then(|r| r.get()) {
        Some(r) => (
            f32::from(r.size.width),
            f32::from(r.size.height),
            f32::from(r.origin.x - raw_origin.x),
            f32::from(r.origin.y - raw_origin.y),
        ),
        None => (w, h, 0.0, 0.0),
    };
    let ox = rdx + rw * group.origin.0 + group.origin_px.0.unwrap_or(0.0);
    let oy = rdy + rh * group.origin.1 + group.origin_px.1.unwrap_or(0.0);
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
    // Изнанка (css-transforms-2 §backface-visibility): элемент разложен и
    // держит место, но не рисуется. m33 — из полной 4×4 самого элемента;
    // у плоских функций он равен 1, так что 2D-путь сюда не попадает.
    // …и по НАКОПЛЕННОЙ, когда элемент внутри объёмного контекста
    // (css-transforms-2 §backface-visibility, «m33 < 0 → not rendered»,
    // где m33 — от накопленной: transform3d-backface-visibility-004/006,
    // backface-visibility-hidden-004, backface-visibility-with-sibling-001).
    // `m33` не зависит ни от сдвигов, ни от свёртки `S·M·S⁻¹`, поэтому
    // считается прямо здесь, до перевода в точки устройства.
    let accum33 = match group.under_3d.as_ref().and_then(|f| f.get()) {
        Some((a, _)) => crate::style::computed::mul4(a, group.m4)[2][2],
        None => group.m4[2][2],
    };
    // Владелец `preserve-3d` изнанкой уносит только СЕБЯ: его дети —
    // отдельные плоскости того же контекста и решают свою видимость сами
    // (composited-under-rotateY-180deg-preserve-3d: зелёный ребёнок под
    // `backface-visibility: hidden; rotateY(180deg); preserve-3d`).
    if group.backface_hidden && accum33 < 0.0 && group.frame_3d.is_none() {
        return;
    }
    // Своя `perspective` (css-transforms-2 §perspective-matrix-computation):
    // T(po)·P(d)·T(−po) в точках устройства — в ячейку для детей ДО их
    // отрисовки, на обоих путях. Свёртка та же, что у объёмного пути ниже:
    // сдвиги ×sf, m34 = −1/(d·sf). Сам элемент перспективой не трогается
    // (она действует только на детей) и идёт своим путём как прежде.
    if let (Some(d), Some(frame)) = (group.perspective, group.perspective_frame.as_ref()) {
        use crate::style::computed::{Transform, mul4};
        let px = group
            .perspective_origin_px
            .0
            .unwrap_or(w * group.perspective_origin.0);
        let py = group
            .perspective_origin_px
            .1
            .unwrap_or(h * group.perspective_origin.1);
        let (px_d, py_d) = (
            (f32::from(raw_origin.x) + px) * scale_factor,
            (f32::from(raw_origin.y) + py) * scale_factor,
        );
        let p = mul4(
            mul4(
                Transform::translate4(px_d, py_d, 0.0),
                Transform::perspective4(d * scale_factor),
            ),
            Transform::translate4(-px_d, -py_d, 0.0),
        );
        frame.set(Some(p));
    }
    // Плоский путь годится, только когда объёмного контекста рядом нет:
    // и владелец `preserve-3d`, и его ребёнок идут по 4×4, даже когда
    // своих объёмных функций у них нет — `m4` держит и плоские функции
    // (`computed::Transform::m4`, «плоские вкладываются как есть»).
    let in_3d = group.frame_3d.is_some() || group.under_3d.as_ref().and_then(|f| f.get()).is_some();
    if !group.has_3d && !in_3d {
        // Плоский путь — прежний по матрице. Маски детей (`overflow`,
        // плитки фона, полосы рамки) едут вместе с содержимым
        // (`Window::with_transformation_masked`): прежде обрезка стояла
        // на месте коробки до `transform` (`transform-clip-001`,
        // `transform-background-001/002`, `transform-fixed-bg-001/003`).
        let quarter = quarter_turn(group.lin)
            .filter(|_| window.current_transformation() == gpui::TransformationMatrix::unit());
        let mut matrix = gpui::TransformationMatrix::unit()
            .translate(origin)
            .compose(gpui::TransformationMatrix {
                rotation_scale: quarter.unwrap_or(group.lin),
                translation: [shift(group.tr[0]), shift(group.tr[1])],
            })
            .translate(back);
        let fill_matrix = quarter
            .map(|_| group.exact_fill_matrix(matrix, bounds.origin, raw_origin, scale_factor));
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
        let child = group.child.as_mut().unwrap();
        window.with_transformation_masked(matrix, |window| {
            if let Some(exact) = fill_matrix {
                window.with_css_fill_transform(exact, |window| child.paint(window, cx));
            } else {
                child.paint(window, cx);
            }
        });
        return;
    }
    // --- Объёмный путь: одна 4×4 ОДНОГО элемента, сплющенная на экран ---
    paint_3d(
        group,
        scale_factor,
        rw,
        rh,
        w,
        h,
        raw_origin,
        ox,
        oy,
        window,
        cx,
    );
}
