//! Проекция и окраска плоскости в объёмном контексте CSS transforms.

use super::plane_spec;
use crate::paint::effects::transformed_element::{Transformed, flatten_plane, invert_affine};
use gpui::{App, Pixels, Window};

#[allow(clippy::too_many_arguments)]
// Coordinate indices address corresponding entries in the projection matrices.
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
    let paint = group.paint.clone();
    use crate::style::computed::{det4, mul4};
    let sf = scale_factor;
    let inputs = Flat3d {
        m4: group.m4,
        m4_pct: group.m4_pct,
        origin_z: group.origin_z,
        reference: (rw, rh),
        size: (w, h),
        raw: [f32::from(raw_origin.x), f32::from(raw_origin.y)],
        origin: (ox, oy),
        scale_factor: sf,
        perspective: group.under_perspective.as_ref().and_then(|f| f.get()),
    };
    let own = inputs.own();
    // Вырожденная 4×4 (`scale3d(2, 2, 0)`, transform3d-scale-004:
    // «singular, causes the contents not to display»).
    if det4(&own).abs() < 1e-9 {
        return;
    }
    // Накопленная матрица объёмного контекста (css-transforms-2
    // §accumulated-3d-transformation-matrix): A(родителя) · P(его
    // перспектива — домножена выше) · C(своя). Обе уже в точках
    // устройства, второй свёртки `S·M·S⁻¹` не возникает — ровно из-за
    // неё «3D full» терял −60.
    let under = group.under_3d.as_ref().and_then(|f| f.get());
    let full = match under {
        Some((a, _)) => mul4(a, own),
        None => own,
    };
    let flat = inputs.flat(&full);
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
    let root_3d = group.frame_3d.is_some() && under.is_none();
    let in_context = root_3d || under.is_some();
    let child = &mut group.child;
    // Plane of a 3D rendering context, or a box under true perspective:
    // painted flat into a buffer and composited through its exact map
    // (`plane_spec`), planes of one context BSP-sorted and split by gpui.
    if in_context || plane_spec::is_projective(&full) {
        let current = window.current_transformation();
        let outer = match under.and_then(|(_, fp)| invert_affine(fp)) {
            Some(inv) => current.compose(inv),
            None => current,
        };
        let x0 = f32::from(raw_origin.x) * sf;
        let y0 = f32::from(raw_origin.y) * sf;
        let spec = plane_spec::plane_spec(&full, flat, outer, (x0, y0, x0 + w * sf, y0 + h * sf));
        let body = |window: &mut Window| {
            window.paint_plane(spec, |window| {
                crate::paint::decorations::outline_phase::scoped(
                    paint.as_ref(),
                    window,
                    cx,
                    |window, cx| child.paint(window, cx),
                )
            })
        };
        if root_3d {
            window.paint_plane_context(body);
        } else {
            body(window);
        }
        return;
    }
    let Some(flat) = flat else {
        return;
    };
    // Маски детей едут за сплющенной матрицей (как на плоском пути,
    // `Window::with_transformation_masked`). Outside a 3D context the map is
    // built from where this box lies alone (`Flat3d`): its primitives keep
    // those inputs, and a scrolled replay builds the moved map by them.
    let unit = window.current_transformation() == gpui::TransformationMatrix::unit();
    let outer = (!unit).then(|| window.current_transform_pivot()).flatten();
    // A parent's perspective is built where the parent lies, which a scroll
    // inside it does not move (a scroller with `perspective`): such a map
    // keeps no pivot, and its scrolled replay builds.
    let pivot = (inputs.perspective.is_none() && (unit || outer.is_some()))
        .then(|| gpui::TransformPivot::built_by(std::rc::Rc::new(inputs), sf, outer));
    super::with_pivot(pivot, window, |window| {
        window.with_transformation_masked(flat, |window| {
            crate::paint::decorations::outline_phase::scoped(
                paint.as_ref(),
                window,
                cx,
                |window, cx| child.paint(window, cx),
            )
        })
    });
}

/// What the flat map of a box's own 4×4 is built from outside a 3D
/// rendering context: the box, its transform and its parent's perspective,
/// in logical pixels where it lies. A scroll move shifts where it lies and
/// builds the map by the operations a fresh paint there runs.
#[derive(Clone, Debug)]
struct Flat3d {
    m4: [[f32; 4]; 4],
    m4_pct: [[[f32; 4]; 4]; 2],
    origin_z: Option<f32>,
    /// The reference box size percentages resolve against.
    reference: (f32, f32),
    size: (f32, f32),
    /// The box origin before device snapping.
    raw: [f32; 2],
    /// The transform origin from `raw`.
    origin: (f32, f32),
    scale_factor: f32,
    /// The parent's perspective matrix (device pixels).
    perspective: Option<[[f32; 4]; 4]>,
}

impl Flat3d {
    /// The element's 4×4 in device pixels, under its parent's perspective.
    fn own(&self) -> [[f32; 4]; 4] {
        use crate::style::computed::{Transform, mul4};
        let (sf, (rw, rh)) = (self.scale_factor, self.reference);
        // Из css-точек в точки устройства — подобие S·M·S⁻¹, S = diag(sf, sf,
        // sf, 1): столбец сдвига строк 0..2 умножается на sf, строка w
        // столбцов 0..2 делится на sf, m44 НЕ трогается. (В шаге 1 цикл `0..4`
        // домножал и m44 — `flatten_plane` делила на него всю матрицу, и
        // каждый объёмный элемент сжимался в 1/sf.)
        let mut own = self.m4;
        for (i, row) in own.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v += rw * self.m4_pct[0][i][j] + rh * self.m4_pct[1][i][j];
            }
        }
        for row in own.iter_mut().take(3) {
            row[3] *= sf;
        }
        for j in 0..3 {
            own[3][j] /= sf;
        }
        // Точка отсчёта по трём осям, в точках устройства: T(o)·M·T(−o).
        let oz = self.origin_z.unwrap_or(0.0) * sf;
        let (ox_d, oy_d) = (
            (self.raw[0] + self.origin.0) * sf,
            (self.raw[1] + self.origin.1) * sf,
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
        match self.perspective {
            Some(p) => mul4(p, own),
            None => own,
        }
    }

    /// The flat map of `full` at the box's centre.
    fn flat(&self, full: &[[f32; 4]; 4]) -> Option<gpui::TransformationMatrix> {
        let (sf, (w, h)) = (self.scale_factor, self.size);
        let center = ((self.raw[0] + w * 0.5) * sf, (self.raw[1] + h * 0.5) * sf);
        // Centre behind the eye: take the tangent map at the mean of the corners
        // that are in front of it (w is linear, so that point is in front too),
        // so the visible part of a plane crossing w = 0 lands in its buffer
        // (perspective-split-by-zero-w).
        flatten_plane(full, center).or_else(|| {
            let (x0, y0) = (self.raw[0] * sf, self.raw[1] * sf);
            let (x1, y1) = (x0 + w * sf, y0 + h * sf);
            let front: Vec<(f32, f32)> = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
                .into_iter()
                .filter(|(x, y)| {
                    full[3][0] * x + full[3][1] * y + full[3][3]
                        > crate::tolerance::HOMOGENEOUS_CLIP_MIN
                })
                .collect();
            if front.is_empty() || front.len() == 4 {
                return None;
            }
            let n = front.len() as f32;
            let p = front
                .iter()
                .fold((0.0, 0.0), |a, b| (a.0 + b.0 / n, a.1 + b.1 / n));
            flatten_plane(full, p)
        })
    }
}

impl gpui::TransformBuild for Flat3d {
    fn matrix(&self) -> gpui::TransformationMatrix {
        self.flat(&self.own()).unwrap_or_else(gpui::TransformationMatrix::unit)
    }

    fn moved(&self, delta: gpui::Point<Pixels>) -> std::rc::Rc<dyn gpui::TransformBuild> {
        let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
        let raw = [self.raw[0] + dx, self.raw[1] + dy];
        std::rc::Rc::new(Flat3d { raw, ..self.clone() })
    }
}
