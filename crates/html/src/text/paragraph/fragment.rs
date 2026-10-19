//! Class-B fragment boundaries from the paragraph's actual line boxes.

use super::Paragraph;
use crate::layout::fragment::flow::{FlowGeometry, FlowMeasurements, FlowNode};
use crate::layout::fragment::types::StackAxis;
use std::rc::Rc;

impl Paragraph {
    pub(crate) fn fragment_flow(
        mut self,
        flow: Option<Rc<FlowMeasurements>>,
        axis: StackAxis,
    ) -> Self {
        self.fragment = flow.filter(|f| f.lines).map(|f| FlowNode::new(f, axis));
        self
    }

    pub(super) fn publish_fragment_lines(&self, pads: &[(f32, f32)], height: f32) {
        if let Some(node) = &self.fragment {
            // CSS Break 3 section 4.1: break between line boxes. The same
            // padding accounts for atoms, emphasis and ruby in the box size.
            *node.geometry.borrow_mut() =
                FlowGeometry::lines(pads.iter().map(|(a, b)| height + a + b));
        }
    }
}
