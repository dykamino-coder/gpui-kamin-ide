//! Source-built raster and homography retained with a scrolling plane.

use super::*;

/// A plane rebuilt from source inputs at its moved spatial location.
pub trait PlaneBuild: std::fmt::Debug {
    /// The resolved plane at this location.
    fn spec(&self) -> PlaneSpec;
    /// Inputs moved in logical pixels, without conjugating a rounded matrix.
    fn moved(&self, delta: Point<Pixels>) -> std::rc::Rc<dyn PlaneBuild>;
}

/// Preserve overflow relative to the plane's source box when it moves.
#[derive(Debug)]
pub(super) struct Ink {
    pub build: std::rc::Rc<dyn PlaneBuild>,
    pub edges: [f32; 4],
}

impl PlaneBuild for Ink {
    fn spec(&self) -> PlaneSpec {
        let mut spec = self.build.spec();
        spec.rect.0 += self.edges[0];
        spec.rect.1 += self.edges[1];
        spec.rect.2 += self.edges[2];
        spec.rect.3 += self.edges[3];
        spec
    }

    fn moved(&self, delta: Point<Pixels>) -> std::rc::Rc<dyn PlaneBuild> {
        std::rc::Rc::new(Self { build: self.build.moved(delta), edges: self.edges })
    }
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
        // The raster's commands move through a fixed clip chain. Translating
        // the old allocation clips ink entering at its far edge; dormant
        // reservoirs may also leave the plane with no drawn ink at all.
        self.bounds = coverage::replayed(&self.scene, self.bounds);
        self.projection = spec.projection;
        self.polygon = match near_clip(&plane_corners(&spec.full, spec.outer, spec.rect)) {
            Some((points, true)) => to_points(&points),
            _ => Vec::new(),
        };
        self.plane = Some((build, *scale));
    }
}
