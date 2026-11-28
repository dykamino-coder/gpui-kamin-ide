//! Source-built accumulated contexts and their outer affine spatial frame.

use super::{flat3d::Flat3d, plane_spec};
use crate::style::computed::{Frame3dBuild, mul4};
use gpui::{Pixels, Point, ScrollMotion, TransformPivot, TransformationMatrix};
use std::rc::Rc;

#[derive(Clone, Debug)]
pub(super) struct PlaneInputs {
    pub inputs: Flat3d,
    pub under: Option<Rc<dyn Frame3dBuild>>,
    pub outer: TransformationMatrix,
    pub outer_pivot: Option<Rc<TransformPivot>>,
    pub motion: ScrollMotion,
}

impl PlaneInputs {
    fn full(&self, inputs: &Flat3d) -> [[f32; 4]; 4] {
        let own = inputs.own();
        self.under
            .as_ref()
            .map_or(own, |under| mul4(under.matrices().0, own))
    }
}

impl Frame3dBuild for PlaneInputs {
    fn matrices(&self) -> ([[f32; 4]; 4], [[f32; 3]; 2]) {
        let mut inputs = self.inputs.clone();
        let delta = self.motion.delta();
        inputs.raw[0] += f32::from(delta.x);
        inputs.raw[1] += f32::from(delta.y);
        let full = self.full(&inputs);
        let m = inputs
            .flat(&full)
            .unwrap_or_else(TransformationMatrix::unit);
        (
            full,
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
            ],
        )
    }

    fn outer(&self) -> TransformationMatrix {
        self.under.as_ref().map_or_else(
            || {
                self.outer_pivot
                    .as_ref()
                    .map_or(self.outer, |pivot| pivot.current_matrix())
            },
            |under| under.outer(),
        )
    }
}

impl gpui::PlaneBuild for PlaneInputs {
    fn spec(&self) -> gpui::PlaneSpec {
        let full = self.full(&self.inputs);
        let sf = self.inputs.scale_factor;
        let (x, y) = (self.inputs.raw[0] * sf, self.inputs.raw[1] * sf);
        let (w, h) = self.inputs.size;
        plane_spec::plane_spec(
            &full,
            self.inputs.flat(&full),
            self.outer(),
            (x, y, x + w * sf, y + h * sf),
        )
    }

    fn moved(&self, delta: Point<Pixels>) -> Rc<dyn gpui::PlaneBuild> {
        let mut inputs = self.inputs.clone();
        inputs.raw[0] += f32::from(delta.x);
        inputs.raw[1] += f32::from(delta.y);
        Rc::new(Self {
            inputs,
            ..self.clone()
        })
    }
}
