//! Проекция и окраска плоскости в объёмном контексте CSS transforms.

mod flat3d;
mod spatial;
use super::plane_spec;
use crate::paint::effects::transformed_element::Transformed;
use flat3d::Flat3d;
use gpui::{App, Pixels, Window};
use spatial::PlaneInputs;

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
    use crate::style::computed::det4;
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
    let under = group.under_3d.as_ref().and_then(|f| f.source());
    let root_3d = group.frame_3d.is_some() && under.is_none();
    let in_context = root_3d || under.is_some();
    let build = std::rc::Rc::new(PlaneInputs {
        inputs: inputs.clone(),
        under,
        outer: window.current_transformation(),
        outer_pivot: window.current_transform_pivot(),
        motion: window.scroll_motion(),
    });
    use crate::style::computed::Frame3dBuild;
    let full = build.matrices().0;
    let flat = inputs.flat(&full);
    if let Some(frame) = group.frame_3d.as_ref() {
        frame.set(build.clone());
    }
    let child = &mut group.child;
    // Plane of a 3D rendering context, or a box under true perspective:
    // painted flat into a buffer and composited through its exact map
    // (`plane_spec`), planes of one context BSP-sorted and split by gpui.
    if in_context || plane_spec::is_projective(&full) {
        use gpui::PlaneBuild;
        let mut spec = build.spec();
        spec.build = Some((build, sf));
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
