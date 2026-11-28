//! Проекция и окраска плоскости в объёмном контексте CSS transforms.

mod flat3d;
use super::plane_spec;
use crate::paint::effects::transformed_element::{Transformed, invert_affine};
use flat3d::Flat3d;
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
        perspective: group
            .under_perspective
            .as_ref()
            .and_then(|f| f.borrow().clone()),
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
        let mut spec =
            plane_spec::plane_spec(&full, flat, outer, (x0, y0, x0 + w * sf, y0 + h * sf));
        let build = PlaneInputs {
            inputs: inputs.clone(),
            under: under.map(|value| value.0),
            outer,
            outer_pivot: window.current_transform_pivot(),
        };
        spec.build = Some((std::rc::Rc::new(build), sf));
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
    // The parent perspective keeps its own spatial source through nested scrolls.
    let pivot = (unit || outer.is_some())
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

/// The plane and its raster keep the same source inputs through a retained move.
#[derive(Clone, Debug)]
struct PlaneInputs {
    inputs: Flat3d,
    under: Option<[[f32; 4]; 4]>,
    outer: gpui::TransformationMatrix,
    outer_pivot: Option<std::rc::Rc<gpui::TransformPivot>>,
}

impl gpui::PlaneBuild for PlaneInputs {
    fn spec(&self) -> gpui::PlaneSpec {
        let own = self.inputs.own();
        let full = self
            .under
            .map_or(own, |under| crate::style::computed::mul4(under, own));
        let sf = self.inputs.scale_factor;
        let (x, y) = (self.inputs.raw[0] * sf, self.inputs.raw[1] * sf);
        let (w, h) = self.inputs.size;
        let outer = self.outer_pivot.as_ref().map_or(self.outer, |p| p.matrix());
        plane_spec::plane_spec(
            &full,
            self.inputs.flat(&full),
            outer,
            (x, y, x + w * sf, y + h * sf),
        )
    }

    fn moved(&self, delta: gpui::Point<Pixels>) -> std::rc::Rc<dyn gpui::PlaneBuild> {
        let mut inputs = self.inputs.clone();
        inputs.raw[0] += delta.x.0;
        inputs.raw[1] += delta.y.0;
        std::rc::Rc::new(Self {
            inputs,
            ..self.clone()
        })
    }
}
