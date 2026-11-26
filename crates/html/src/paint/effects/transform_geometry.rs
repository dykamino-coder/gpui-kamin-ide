//! Exact transform origins before device snapping (CSS Transforms 1 §3): the
//! unrounded origin of a diagonal scale, and the linear part classified as a
//! quarter turn or reflection, whose edges snap their own way.

use crate::paint::effects::transformed_element::Transformed;
use gpui::{Pixels, Point};

impl Transformed {
    pub(crate) fn scaled_origin(&self, origin: Point<Pixels>) -> Point<Pixels> {
        let [[a, b], [c, d]] = self.lin;
        let eps = 1e-5;
        // The translation path already carries unrounded placement, and the
        // quarter-turn path has its own final edge snapping. Only diagonal
        // scaling (including reflections) needs this origin before the quad
        // is snapped on the device.
        let scaled = b.abs() < eps
            && c.abs() < eps
            && a.abs() > eps
            && d.abs() > eps
            && ((a - 1.0).abs() > eps || (d - 1.0).abs() > eps)
            && quarter_turn(self.lin).is_none();
        if scaled {
            self.exact_origin.unwrap_or(origin)
        } else {
            origin
        }
    }
}

/// Линейная часть — поворот на кратное 90° или отражение (знаковая
/// перестановка) с точностью до ошибки `f32`, но НЕ единичная: точная
/// матрица из 0/±1.
pub(super) fn quarter_turn(lin: [[f32; 2]; 2]) -> Option<[[f32; 2]; 2]> {
    let unit = |v: f32| {
        [-1.0f32, 0.0, 1.0]
            .into_iter()
            .find(|u| (v - u).abs() < 1e-5)
    };
    let m = [
        [unit(lin[0][0])?, unit(lin[0][1])?],
        [unit(lin[1][0])?, unit(lin[1][1])?],
    ];
    let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
    (det.abs() == 1.0 && m != [[1.0, 0.0], [0.0, 1.0]]).then_some(m)
}
