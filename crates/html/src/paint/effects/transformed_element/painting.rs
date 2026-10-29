//! Окраска плоского и объёмного преобразования поддерева.

mod pieces;
mod plane;
mod plane_spec;
use pieces::pieces;
mod projection;
pub(super) use projection::paint_3d;

use crate::paint::effects::transformed_element::Transformed;
use gpui::{App, Bounds, LayoutId, Pixels, Window};

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_body(
    group: &mut Transformed,
    bounds: Bounds<Pixels>,
    layout_id: &mut LayoutId,
    _prepaint: &mut (),
    window: &mut Window,
    cx: &mut App,
) {
    let paint = group.paint.clone();
    if group.placed {
        crate::paint::decorations::outline_phase::scoped(
            paint.as_ref(),
            window,
            cx,
            |window, cx| {
                group.child.paint(window, cx);
            },
        );
        return;
    }
    let scale_factor = window.scale_factor();
    // CSS transform origins precede device-pixel snapping (Transforms 1 §3).
    let raw_origin = group.exact_origin.unwrap_or(bounds.origin);
    let scaled_origin = group.scaled_origin(bounds.origin);
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
    let fragment = crate::layout::fragment::effect_fragment::reference_box(
        paint.as_deref(),
        Bounds {
            origin: raw_origin,
            size: exact,
        },
    );
    let r = plane::reference(group, raw_origin, exact, fragment);
    let (rw, rh) = (r.0, r.1);
    let (ox, oy) = plane::origin(group, r);
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
        let unit = window.current_transformation() == gpui::TransformationMatrix::unit();
        let geometry = (bounds, raw_origin, scaled_origin);
        let (matrix, fill_matrix) = plane::matrix(group, geometry, exact, r, (scale_factor, unit));
        if let Some(all) =
            crate::layout::fragment::effect_fragment::pieces::active(paint.as_deref())
        {
            return pieces(
                group,
                &all,
                (geometry, exact, matrix),
                (scale_factor, unit),
                window,
                cx,
            );
        }
        let child = &mut group.child;
        crate::layout::fragment::effect_fragment::paint(
            paint.as_deref(),
            matrix,
            fill_matrix.unwrap_or(matrix),
            group.fragment_clip_ancestor,
            window,
            |window| {
                if let Some(exact) = fill_matrix {
                    let mut frame = window.raster_frame().clone();
                    frame.fill_transform = Some((window.current_transformation(), exact));
                    window.with_raster_frame(frame, |window| {
                        crate::paint::decorations::outline_phase::scoped(
                            paint.as_ref(),
                            window,
                            cx,
                            |window, cx| child.paint(window, cx),
                        )
                    });
                } else {
                    crate::paint::decorations::outline_phase::scoped(
                        paint.as_ref(),
                        window,
                        cx,
                        |window, cx| child.paint(window, cx),
                    );
                }
            },
        );
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
