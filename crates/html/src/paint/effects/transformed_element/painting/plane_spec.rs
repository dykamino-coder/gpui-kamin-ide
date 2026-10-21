//! Точная проекция плоскости объёмного контекста: аффинная карта растра плюс остаточная
//! гомография для `Window::paint_plane`.
//!
//! css-transforms-2 §3d-transform-rendering: плоскость объёмного контекста (или коробка под
//! перспективой) рисуется плоско и отображается своей накопленной 4×4. gpui красит аффинно,
//! поэтому содержимое рисуется под касательной аффинной картой `A` плоскости (точной, когда
//! карта аффинна — 2D-эквивалентные плоскости остаются попиксельно теми же), а буфер
//! компонуется через остаточную гомографию `H·A⁻¹`, как viz рисует текстуру слоя полной
//! матрицей (`draw_polygon.cc`). Шум `f32` в карте растра снимается до сетки пикселей.

use gpui::{PlaneSpec, TransformationMatrix};

type M3 = [[f32; 3]; 3];

fn affine3(m: TransformationMatrix) -> M3 {
    let r = m.rotation_scale;
    let t = m.translation;
    [
        [r[0][0], r[0][1], t[0]],
        [r[1][0], r[1][1], t[1]],
        [0.0, 0.0, 1.0],
    ]
}

fn mul3(a: &M3, b: &M3) -> M3 {
    let mut m = [[0.0f32; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    m
}

fn inv3(m: &M3) -> Option<M3> {
    let c =
        |r0: usize, r1: usize, c0: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
    let det = m[0][0] * c(1, 2, 1, 2) - m[0][1] * c(1, 2, 0, 2) + m[0][2] * c(1, 2, 0, 1);
    if det.abs() < 1e-12 {
        return None;
    }
    let adj = [
        [c(1, 2, 1, 2), -c(0, 2, 1, 2), c(0, 1, 1, 2)],
        [-c(1, 2, 0, 2), c(0, 2, 0, 2), -c(0, 1, 0, 2)],
        [c(1, 2, 0, 1), -c(0, 2, 0, 1), c(0, 1, 0, 1)],
    ];
    Some(adj.map(|row| row.map(|v| v / det)))
}

/// Remove float noise from the raster map: a chain like `rotate3d(1,1,1,90deg)`
/// · `rotate3d(1,1,1,-90deg)` is the identity up to ~1e-7, and the residue
/// put every box edge a hair off the pixel grid (half-covered edge pixels).
/// Like cc's approximately-integer-translation checks, entries within 1e-5
/// of −1/0/1 and translations within 1e-3 px of an integer are snapped.
fn snap(m: TransformationMatrix) -> TransformationMatrix {
    let near = |v: f32, eps: f32| {
        let r = v.round();
        if (v - r).abs() < eps { r } else { v }
    };
    let lin = m.rotation_scale.map(|row| row.map(|v| near(v, 1e-5)));
    let axis = lin.iter().flatten().all(|v| v.fract() == 0.0);
    TransformationMatrix {
        rotation_scale: lin,
        translation: if axis {
            m.translation.map(|v| near(v, 1e-3))
        } else {
            m.translation
        },
    }
}

/// Whether the plane map `full` is projective (w depends on x or y).
pub(super) fn is_projective(full: &[[f32; 4]; 4]) -> bool {
    full[3][0].abs() > 1e-9 || full[3][1].abs() > 1e-9
}

/// Plane description for `Window::paint_plane`.
///
/// `full` maps the plane's device points (z = 0) to the context space,
/// `outer` is the affine transform active outside the context, `flat` the
/// tangent affine raster map (`None`: centre edge-on or behind the eye),
/// `rect` the box in device points.
pub(super) fn plane_spec(
    full: &[[f32; 4]; 4],
    flat: Option<TransformationMatrix>,
    outer: TransformationMatrix,
    rect: (f32, f32, f32, f32),
) -> PlaneSpec {
    let o = affine3(outer);
    // Centre behind the eye (or edge-on): no tangent map there; paint the
    // content untransformed and let the full homography place what is in
    // front of the eye (perspective-split-by-zero-w). Edge-on maps have no
    // inverse — nothing of the plane is drawn.
    let raster = match flat {
        Some(flat) => snap(outer.compose(flat)),
        None => outer,
    };
    let h = [
        [full[0][0], full[0][1], full[0][3]],
        [full[1][0], full[1][1], full[1][3]],
        [full[3][0], full[3][1], full[3][3]],
    ];
    let projection = if is_projective(full) || flat.is_none() {
        inv3(&mul3(&o, &h))
            .map(|hinv| {
                let p = mul3(&affine3(raster), &hinv);
                [p[0], p[1], p[2]].concat().try_into().unwrap_or([0.0; 9])
            })
            .or(Some([0.0; 9]))
    } else {
        None
    };
    PlaneSpec {
        raster,
        projection,
        full: *full,
        outer,
        rect,
        masked: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affine_plane_has_no_residual() {
        let mut full = [[0.0f32; 4]; 4];
        for (i, row) in full.iter_mut().enumerate() {
            row[i] = 1.0;
        }
        full[0][3] = 5.0;
        let flat = TransformationMatrix {
            rotation_scale: [[1.0, 0.0], [0.0, 1.0]],
            translation: [5.0, 0.0],
        };
        let spec = plane_spec(
            &full,
            Some(flat),
            TransformationMatrix::unit(),
            (0.0, 0.0, 10.0, 10.0),
        );
        assert!(spec.projection.is_none());
        assert_eq!(spec.rect, (0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn inverse_of_product_round_trips() {
        let m = [[2.0, 0.5, 3.0], [0.1, 1.5, -2.0], [0.001, 0.002, 1.0]];
        let inv = inv3(&m).unwrap();
        let id = mul3(&m, &inv);
        for (i, row) in id.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                let want = if i == j { 1.0 } else { 0.0 };
                assert!((v - want).abs() < 1e-4);
            }
        }
    }
}
