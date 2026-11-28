//! Perspective resolved from the parent's own spatial frame.

use super::{Transform, mul4};
use gpui::{Pixels, Point, ScrollMotion};

/// Inputs remain attached to the parent's frame when a child scrolls inside it.
#[derive(Clone, Debug, PartialEq)]
pub struct PerspectiveInputs {
    pub distance: f32,
    pub base: Point<Pixels>,
    pub origin: (f32, f32),
    pub scale: f32,
    pub motion: ScrollMotion,
}

impl PerspectiveInputs {
    /// Build exactly as a fresh paint at the parent's current location does.
    pub fn matrix(&self) -> [[f32; 4]; 4] {
        let delta = self.motion.delta();
        let x = (f32::from(self.base.x) + f32::from(delta.x) + self.origin.0) * self.scale;
        let y = (f32::from(self.base.y) + f32::from(delta.y) + self.origin.1) * self.scale;
        mul4(
            mul4(
                Transform::translate4(x, y, 0.0),
                Transform::perspective4(self.distance * self.scale),
            ),
            Transform::translate4(-x, -y, 0.0),
        )
    }
}
