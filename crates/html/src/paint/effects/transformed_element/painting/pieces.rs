//! Плоское преобразование внутри записи главной копии разрезанного ребёнка: в каждом куске
//! своя матрица — вокруг опорной коробки его фрагмента (css-break-3 §5.5), как у копии.

use super::plane;
use crate::layout::fragment::effect_fragment::pieces::{Pieces, paint_transformed};
use crate::paint::effects::transformed_element::Transformed;
use gpui::{App, Bounds, Pixels, Point, Size, TransformationMatrix, Window};

type Geometry = (Bounds<Pixels>, Point<Pixels>, Point<Pixels>);

pub(super) fn pieces(
    group: &mut Transformed,
    all: &Pieces,
    (geometry, exact, matrix): (Geometry, Size<Pixels>, TransformationMatrix),
    snap: (f32, bool),
    window: &mut Window,
    cx: &mut App,
) {
    let raw_origin = geometry.1;
    let own = Bounds {
        origin: raw_origin,
        size: exact,
    };
    let matrices: Vec<TransformationMatrix> = all
        .pieces
        .iter()
        .map(|p| {
            let r = plane::reference(group, raw_origin, exact, Some(p.reference(own)));
            plane::matrix(group, geometry, exact, r, snap).0
        })
        .collect();
    let [[a, b], [c, d]] = matrix.rotation_scale;
    let axes = (b.abs() < 1e-4 && c.abs() < 1e-4) || (a.abs() < 1e-4 && d.abs() < 1e-4);
    let clip_before = axes && !group.fragment_clip_ancestor;
    let paint = group.paint.clone();
    let child = &mut group.child;
    paint_transformed(all, &matrices, clip_before, (window, cx), |window, cx| {
        crate::paint::decorations::outline_phase::scoped(
            paint.as_ref(),
            window,
            cx,
            |window, cx| child.paint(window, cx),
        )
    });
}
