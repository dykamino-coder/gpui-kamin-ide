//! Source-built raster and homography retained with a scrolling plane.

use super::*;

/// A plane rebuilt from source inputs at its moved spatial location.
pub trait PlaneBuild: std::fmt::Debug {
    /// The resolved plane at this location.
    fn spec(&self) -> PlaneSpec;
    /// Inputs moved in logical pixels, without conjugating a rounded matrix.
    fn moved(&self, delta: Point<Pixels>) -> std::rc::Rc<dyn PlaneBuild>;
}

#[derive(Debug)]
pub(super) struct Raster(pub std::rc::Rc<dyn PlaneBuild>);

impl crate::TransformBuild for Raster {
    fn matrix(&self) -> TransformationMatrix { self.0.spec().raster }
    fn moved(&self, delta: Point<Pixels>) -> std::rc::Rc<dyn crate::TransformBuild> {
        std::rc::Rc::new(Self(self.0.moved(delta)))
    }
}

impl PaintGroup {
    /// Rebuild the residual homography from the same inputs as the raster.
    pub(crate) fn move_plane(&mut self, delta: Point<ScaledPixels>) {
        let Some((build, scale)) = &self.plane else { return };
        let build = build.moved(delta.map(|v| Pixels(v.0 / scale)));
        let spec = build.spec();
        self.projection = spec.projection;
        if !self.polygon.is_empty() {
            self.polygon = near_clip(&plane_corners(&spec.full, spec.outer, spec.rect))
                .map(|(points, _)| to_points(&points)).unwrap_or_default();
        }
        self.plane = Some((build, *scale));
    }
}
