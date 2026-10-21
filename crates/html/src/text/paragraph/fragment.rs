//! Границы фрагментов класса B по фактическим строковым боксам абзаца.

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
            let mut geometry = FlowGeometry::lines(pads.iter().map(|(a, b)| height + a + b));
            // CSS 2.1 section 9.5: avoidance is free space before a real
            // line, so a fragment cut may lie at the end of that space.
            for (solid, line) in geometry.solid.iter_mut().zip(&self.lines) {
                solid.0 += f32::from(line.clearance);
                if line.clearance > gpui::px(0.) {
                    geometry.cuts.push((solid.0, solid.0));
                }
            }
            *node.geometry.borrow_mut() = geometry;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::paragraph::{Align, Line, Wrap};
    use gpui::px;

    #[test]
    fn float_avoidance_space_can_split_between_line_grid_positions() {
        let mut paragraph = Paragraph::new(
            "ab".into(),
            Vec::new(),
            px(16.0),
            px(16.0),
            Align::Left,
            Wrap::default(),
        );
        paragraph.lines = [0..1, 1..2]
            .into_iter()
            .enumerate()
            .map(|(i, range)| Line {
                clearance: px(if i == 1 { 32. } else { 0. }),
                range,
                width: px(0.),
                ellipsis: false,
                clamped: false,
                vis_cut: None,
                hyphen: false,
                indent: px(0.),
            })
            .collect();
        let node = FlowNode::new(
            Rc::new(FlowMeasurements::for_lines()),
            StackAxis::Horizontal,
        );
        paragraph.fragment = Some(node.clone());
        paragraph.publish_fragment_lines(&[(0., 0.), (32., 0.)], 16.);
        let geometry = node.geometry.borrow();
        assert_eq!(geometry.solid, [(0., 16.), (48., 64.)]);
        assert!(geometry.cuts.contains(&(48., 48.)));
        assert!(geometry.cuts.contains(&(16., 16.)));
        assert!(geometry.cuts.contains(&(64., 64.)));
    }
}
