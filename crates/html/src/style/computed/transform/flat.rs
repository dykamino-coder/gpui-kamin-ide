//! Плоское преобразование коробки, решаемое от опорной коробки: так `transform-origin` и
//! доли сдвига берутся от каждого фрагмента отдельно (css-transforms-1 §transform-rendering,
//! css-break-3 §5.5), а не от непрерывной коробки.

use super::super::Computed;
use gpui::{Bounds, Pixels, TransformationMatrix};

/// The 2D matrix of a box beyond a translation, resolvable per reference box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FlatTransform {
    lin: [[f32; 2]; 2],
    tr: [[f32; 3]; 2],
    origin: (f32, f32),
    origin_px: (Option<f32>, Option<f32>),
}

impl FlatTransform {
    /// The box's transform when it is flat (no 3D context, perspective or depth)
    /// and its linear part is not the identity; `None` for a pure translation.
    pub(crate) fn of(c: &Computed) -> Option<Self> {
        let t = c.transform?.after_individual(c.rotate_prop, c.scale_prop);
        if t.has_3d || c.perspective.is_some() || c.preserve_3d == Some(true) {
            return None;
        }
        let [[a, b], [cc, d]] = t.lin;
        let eps = 1e-5;
        let identity =
            (a - 1.).abs() < eps && b.abs() < eps && cc.abs() < eps && (d - 1.).abs() < eps;
        if identity {
            return None;
        }
        let mut tr = t.tr;
        // A pure px shift already moved the box in layout (`folded_shift`).
        if c.folded_shift().is_some() {
            tr[0][0] = 0.;
            tr[1][0] = 0.;
        }
        Some(Self {
            lin: t.lin,
            tr,
            origin: c.transform_origin.unwrap_or((0.5, 0.5)),
            origin_px: c.transform_origin_px,
        })
    }

    /// The matrix (logical pixels) about `reference`, the box or one fragment.
    pub(crate) fn matrix(&self, reference: Bounds<Pixels>) -> TransformationMatrix {
        let (x, y) = (f32::from(reference.origin.x), f32::from(reference.origin.y));
        let (w, h) = (
            f32::from(reference.size.width),
            f32::from(reference.size.height),
        );
        let ox = x + w * self.origin.0 + self.origin_px.0.unwrap_or(0.);
        let oy = y + h * self.origin.1 + self.origin_px.1.unwrap_or(0.);
        let sx = self.tr[0][0] + w * self.tr[0][1] + h * self.tr[0][2];
        let sy = self.tr[1][0] + w * self.tr[1][1] + h * self.tr[1][2];
        let [[a, b], [c, d]] = self.lin;
        TransformationMatrix {
            rotation_scale: self.lin,
            translation: [ox - a * ox - b * oy + sx, oy - c * ox - d * oy + sy],
        }
    }
}
