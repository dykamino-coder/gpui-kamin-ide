//! Add measured float-flow opportunities to the common column input.

use crate::layout::fragment::types::{StackAxis, StackChild};
use gpui::{Pixels, Size, Window, point, px};

impl StackChild {
    pub(crate) fn measure_flow(
        &mut self,
        size: Size<Pixels>,
        axis: StackAxis,
        window: &mut Window,
    ) {
        if let Some(flow) = &self.flow {
            let geometry = flow.at(point(px(0.0), px(0.0)), size, axis, window);
            geometry.merge(&mut self.cuts, &mut self.solid);
        }
    }
}
