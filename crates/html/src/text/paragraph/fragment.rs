//! Границы фрагментов класса B по фактическим строковым боксам абзаца.

use super::Paragraph;
use crate::layout::fragment::flow::{FlowGeometry, FlowMeasure, FlowNode};
use crate::layout::fragment::types::StackAxis;
use std::rc::Rc;

/// The paragraph's line boxes in a measured fragmentation flow, with the
/// `orphans`/`widows` of its block container (css-break-3 §4.4).
#[derive(Clone, Debug)]
pub(crate) struct LineFlow {
    pub(crate) node: FlowNode,
    keep: (usize, usize),
}

impl Paragraph {
    /// Consumed fragmentainer space belongs above the continued line box.
    pub(super) fn fragment_padding(&self, mut pads: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
        if let Some(fragment) = &self.fragment {
            for ((above, _), growth) in pads.iter_mut().zip(fragment.node.growth.borrow().iter()) {
                *above += growth;
            }
        }
        pads
    }

    pub(crate) fn fragment_flow(
        mut self,
        flow: Option<Rc<FlowMeasure>>,
        axis: StackAxis,
        keep: (usize, usize),
    ) -> Self {
        self.fragment = flow.filter(|f| f.lines).map(|f| LineFlow {
            node: FlowNode::new(f, axis),
            keep,
        });
        self
    }

    pub(super) fn publish_fragment_lines(&self, pads: &[(f32, f32)], height: f32) {
        if let Some(LineFlow { node, keep }) = &self.fragment {
            // CSS Break 3 section 4.1: break between line boxes. The same
            // padding accounts for atoms, emphasis and ruby in the box size.
            let mut geometry = FlowGeometry::lines(pads.iter().map(|(a, b)| height + a + b));
            // CSS 2.1 section 9.5: avoidance is free space before a real
            // line, so a fragment cut may lie at the end of that space.
            let growth = node.growth.borrow();
            let mut starts = node.starts.borrow_mut();
            starts.clear();
            for (index, (solid, line)) in geometry.solid.iter_mut().zip(&self.lines).enumerate() {
                let consumed = growth.get(index).copied().unwrap_or(0.0);
                starts.push(solid.0 + consumed);
                solid.0 += consumed;
                solid.0 += f32::from(line.clearance);
                if line.clearance > gpui::px(0.) || consumed > 0.0 {
                    geometry.cuts.push((solid.0, solid.0));
                }
            }
            geometry.keep_lines(keep.0, keep.1);
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
        let node = FlowNode::new(Rc::new(FlowMeasure::for_lines()), StackAxis::Horizontal);
        paragraph.fragment = Some(LineFlow {
            node: node.clone(),
            keep: (1, 1),
        });
        paragraph.publish_fragment_lines(&[(0., 0.), (32., 0.)], 16.);
        let geometry = node.geometry.borrow();
        assert_eq!(geometry.solid, [(0., 16.), (48., 64.)]);
        assert!(geometry.cuts.contains(&(48., 48.)));
        assert!(geometry.cuts.contains(&(16., 16.)));
        assert!(geometry.cuts.contains(&(64., 64.)));
    }
    #[test]
    fn consumed_space_moves_only_the_continuation_and_survives_rebuild() {
        let flow = Rc::new(FlowMeasurements::for_lines());
        let mut paragraph = Paragraph::new(
            "ab".into(),
            Vec::new(),
            px(20.),
            px(20.),
            Align::Left,
            Wrap::default(),
        );
        paragraph.lines = [0..1, 1..2]
            .into_iter()
            .map(|range| Line {
                clearance: px(0.),
                range,
                width: px(0.),
                ellipsis: false,
                clamped: false,
                vis_cut: None,
                hyphen: false,
                indent: px(0.),
            })
            .collect();
        paragraph = paragraph.fragment_flow(Some(flow.clone()), StackAxis::Horizontal, (1, 1));
        let node = paragraph.fragment.as_ref().unwrap().node.clone();
        let push = crate::layout::fragment::flow::LinePush {
            at: 20.,
            line: 1,
            growth: node.growth.clone(),
        };
        assert!(push.grow(15.));
        let pads = paragraph.line_padding();
        paragraph.publish_fragment_lines(&pads, 20.);
        assert_eq!(pads, [(0., 0.), (15., 0.)]);
        assert_eq!(node.geometry.borrow().solid, [(0., 20.), (35., 55.)]);
        assert!(node.geometry.borrow().cuts.contains(&(35., 35.)));
        flow.rewind();
        let rebuilt = FlowNode::new(flow.clone(), StackAxis::Horizontal);
        assert!(Rc::ptr_eq(&rebuilt.growth, &node.growth));
        let spare = FlowNode::new(Rc::new(flow.constraints_only()), StackAxis::Horizontal);
        assert!(Rc::ptr_eq(&spare.growth, &node.growth));
    }
}
