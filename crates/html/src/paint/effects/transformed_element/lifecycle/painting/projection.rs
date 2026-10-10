//! Проекция и окраска плоскости в объёмном контексте CSS transforms.

use crate::paint::effects::transformed_element::{Transformed, flatten_plane, invert_affine};
use gpui::{AnyElement, App, Pixels, Window};

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_3d(
    group: &mut Transformed,
    scale_factor: f32,
    rw: f32,
    rh: f32,
    w: f32,
    h: f32,
    raw_origin: gpui::Point<Pixels>,
    ox: f32,
    oy: f32,
    window: &mut Window,
    cx: &mut App,
) {
    use crate::style::computed::{Transform, det4, mul4};
    let sf = scale_factor;
    // Из css-точек в точки устройства — подобие S·M·S⁻¹, S = diag(sf, sf,
    // sf, 1): столбец сдвига строк 0..2 умножается на sf, строка w
    // столбцов 0..2 делится на sf, m44 НЕ трогается. (В шаге 1 цикл `0..4`
    // домножал и m44 — `flatten_plane` делила на него всю матрицу, и
    // каждый объёмный элемент сжимался в 1/sf; scout-3d-2026-09b.md §1.)
    let mut own = group.m4;
    for i in 0..3 {
        own[i][3] = (own[i][3] + rw * group.m4_pct[i][0] + rh * group.m4_pct[i][1]) * sf;
    }
    for j in 0..3 {
        own[3][j] /= sf;
    }
    // Точка отсчёта по трём осям, в точках устройства: T(o)·M·T(−o).
    // `ox`/`oy` посчитаны выше в css-точках; `ScaledPixels.0` — pub(crate)
    // в gpui, поэтому `origin.x.0` отсюда не читается.
    let oz = group.origin_z.unwrap_or(0.0) * sf;
    let (ox_d, oy_d) = (
        (f32::from(raw_origin.x) + ox) * sf,
        (f32::from(raw_origin.y) + oy) * sf,
    );
    let own = mul4(
        mul4(Transform::translate4(ox_d, oy_d, oz), own),
        Transform::translate4(-ox_d, -oy_d, -oz),
    );
    // Перспектива ПРЯМОГО родителя (§3d-transform-rendering, п.3:
    // «pre-multiply the parent element's perspective matrix»); стека
    // preserve-3d здесь ещё нет — внукам не достаётся
    // (perspective-children-only-*). Ячейку наполнил `paint` родителя в
    // этом же кадре (или прошлом — она переживает кадр), поэтому её видит
    // и отложенный слой абсолюта. Плоский ребёнок (z = 0) под
    // перспективой не меняется — потому только объёмный путь.
    let own = match group.under_perspective.as_ref().and_then(|f| f.get()) {
        Some(p) => mul4(p, own),
        None => own,
    };
    // Вырожденная 4×4 (`scale3d(2, 2, 0)`, transform3d-scale-004:
    // «singular, causes the contents not to display»).
    if det4(&own).abs() < 1e-9 {
        return;
    }
    // Накопленная матрица объёмного контекста (css-transforms-2
    // §accumulated-3d-transformation-matrix): A(родителя) · P(его
    // перспектива — домножена выше) · C(своя). Обе уже в точках
    // устройства, второй свёртки `S·M·S⁻¹` не возникает — ровно из-за
    // неё «3D full» терял −60 (computed.rs:691).
    let under = group.under_3d.as_ref().and_then(|f| f.get());
    let full = match under {
        Some((a, _)) => mul4(a, own),
        None => own,
    };
    let center = (
        (f32::from(raw_origin.x) + w * 0.5) * sf,
        (f32::from(raw_origin.y) + h * 0.5) * sf,
    );
    let flat = flatten_plane(&full, center);
    // Ячейка для СВОИХ детей — накопленная и своя аффинная доля;
    // наполняется ДО отрисовки детей, как у перспективы (и ради
    // отложенных слоёв абсолютов).
    if let Some(frame) = group.frame_3d.as_ref() {
        let share = flat.map_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], |m| {
            [
                [
                    m.rotation_scale[0][0],
                    m.rotation_scale[0][1],
                    m.translation[0],
                ],
                [
                    m.rotation_scale[1][0],
                    m.rotation_scale[1][1],
                    m.translation[1],
                ],
            ]
        });
        frame.set(Some((full, share)));
    }
    // Plane depth in the 3D rendering context (css-transforms-2
    // §3d-transform-rendering: planes of one context render by z, not
    // document order) — z/w of the box centre under the accumulated
    // matrix; the context root opens the sorting scope.
    let depth = {
        let v = [center.0, center.1, 0.0, 1.0];
        let row = |i: usize| (0..4).map(|k| full[i][k] * v[k]).sum::<f32>();
        let (z, wv) = (row(2), row(3));
        if wv.abs() > 1e-6 { z / wv } else { z }
    };
    let root_3d = group.frame_3d.is_some() && under.is_none();
    let in_context = root_3d || under.is_some();
    let paint_child = |child: &mut AnyElement,
                       m: Option<gpui::TransformationMatrix>,
                       masked: bool,
                       window: &mut Window,
                       cx: &mut App| {
        if in_context {
            let mut body = |window: &mut Window| {
                window.paint_depth_plane(depth, |window| match m {
                    Some(m) if masked => {
                        window.with_transformation_masked(m, |window| child.paint(window, cx))
                    }
                    Some(m) => window.with_transformation(m, |window| child.paint(window, cx)),
                    None => child.paint(window, cx),
                })
            };
            if root_3d {
                window.paint_depth_context(body)
            } else {
                body(window)
            }
        } else {
            match m {
                Some(m) if masked => {
                    window.with_transformation_masked(m, |window| child.paint(window, cx))
                }
                Some(m) => window.with_transformation(m, |window| child.paint(window, cx)),
                None => child.paint(window, cx),
            }
        }
    };
    // Ребро (`rotateX(90deg)`) — не рисуется, как и прежняя нулевая
    // высота. Но в объёмном контексте ПОТОМКИ ребром не становятся
    // (transform3d-preserve3d-011: `rotateX(90)` над `rotateX(90)` =
    // 180°): краска идёт под единичной долей, а место каждый потомок
    // назначает себе сам по накопленной.
    let Some(flat) = flat else {
        if group.frame_3d.is_some() {
            let child = group.child.as_mut().unwrap();
            paint_child(
                child,
                Some(gpui::TransformationMatrix::unit()),
                false,
                window,
                cx,
            );
        }
        return;
    };
    // Своя доля для gpui: родитель УЖЕ втолкнул `F_P`, а вложения
    // складываются как `inner∘outer` (`window.rs:2789`, порядок замерен и
    // оставлен) — значит втолкнуть надо `G ∘ F_P⁻¹`.
    let flat = match under.and_then(|(_, fp)| invert_affine(fp)) {
        Some(inv) => flat.compose(inv),
        None => flat,
    };
    let child = group.child.as_mut().unwrap();
    // Маски детей едут за сплющенной матрицей (как на плоском пути,
    // `Window::with_transformation_masked`); косая — прежнее поведение.
    paint_child(child, Some(flat), true, window, cx);
}
