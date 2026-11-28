//! Source inputs for a retained 3D raster transform.
use crate::paint::effects::transformed_element::flatten_plane;
use gpui::Pixels;

/// What the flat map of a box's own 4×4 is built from outside a 3D
/// rendering context: the box, its transform and its parent's perspective,
/// in logical pixels where it lies. A scroll move shifts where it lies and
/// builds the map by the operations a fresh paint there runs.
#[derive(Clone, Debug)]
pub(super) struct Flat3d {
    pub(super) m4: [[f32; 4]; 4],
    pub(super) m4_pct: [[[f32; 4]; 4]; 2],
    pub(super) origin_z: Option<f32>,
    /// The reference box size percentages resolve against.
    pub(super) reference: (f32, f32),
    pub(super) size: (f32, f32),
    /// The box origin before device snapping.
    pub(super) raw: [f32; 2],
    /// The transform origin from `raw`.
    pub(super) origin: (f32, f32),
    pub(super) scale_factor: f32,
    /// The parent's perspective matrix (device pixels).
    pub(super) perspective: Option<crate::style::computed::PerspectiveInputs>,
}

impl Flat3d {
    /// The element's 4×4 in device pixels, under its parent's perspective.
    pub(super) fn own(&self) -> [[f32; 4]; 4] {
        use crate::style::computed::{Transform, mul4};
        let (sf, (rw, rh)) = (self.scale_factor, self.reference);
        // CSS px to device px is the similarity S·M·S⁻¹, S = diag(sf, sf, sf,
        // 1): the translation column of rows 0..2 is multiplied by sf, the w
        // row of columns 0..2 divided by sf, and m44 is left alone
        // (`flatten_plane` divides the whole matrix by it: scaling it shrank
        // every 3D box by 1/sf).
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
        // The origin on three axes, in device px: T(o)·M·T(−o).
        let oz = self.origin_z.unwrap_or(0.0) * sf;
        let (ox_d, oy_d) = (
            (self.raw[0] + self.origin.0) * sf,
            (self.raw[1] + self.origin.1) * sf,
        );
        let own = mul4(
            mul4(Transform::translate4(ox_d, oy_d, oz), own),
            Transform::translate4(-ox_d, -oy_d, -oz),
        );
        // The DIRECT parent's perspective (§3d-transform-rendering, step 3:
        // "pre-multiply the parent element's perspective matrix"); with no
        // preserve-3d stack here, grandchildren do not get it
        // (perspective-children-only-*). The parent's `paint` filled the cell
        // in this frame (or an earlier one: it outlives the frame), so a
        // deferred absolute layer sees it too. A flat child (z = 0) is not
        // changed by a perspective, hence the 3D path only.
        match &self.perspective {
            Some(p) => mul4(p.matrix(), own),
            None => own,
        }
    }

    /// The flat map of `full` at the box's centre.
    pub(super) fn flat(&self, full: &[[f32; 4]; 4]) -> Option<gpui::TransformationMatrix> {
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
        self.flat(&self.own())
            .unwrap_or_else(gpui::TransformationMatrix::unit)
    }

    fn moved(&self, delta: gpui::Point<Pixels>) -> std::rc::Rc<dyn gpui::TransformBuild> {
        let (dx, dy) = (f32::from(delta.x), f32::from(delta.y));
        let raw = [self.raw[0] + dx, self.raw[1] + dy];
        std::rc::Rc::new(Flat3d {
            raw,
            ..self.clone()
        })
    }
}
