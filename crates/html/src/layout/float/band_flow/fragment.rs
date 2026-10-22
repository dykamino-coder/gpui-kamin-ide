//! Геометрия разрывов измеренного плана band до того, как временные деревья истекут.

use super::{Kid, Plan, Slot};
use crate::layout::fragment::flow::{FloatGeometry, FlowGeometry, FlowMeasurements, FlowNode};
use crate::layout::fragment::types::StackAxis;
use std::rc::Rc;

impl super::BandFlow {
    pub(crate) fn fragment_flow(
        mut self,
        parent: Rc<FlowMeasurements>,
        probes: Rc<FlowMeasurements>,
        axis: StackAxis,
    ) -> Self {
        self.fragment = Some(FragmentFlow::new(parent, probes, axis));
        self
    }
}

impl<'a> super::Probe<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        window: &'a mut gpui::Window,
        cx: &'a mut gpui::App,
        vert: bool,
        fragment: Option<&FragmentFlow>,
        paint: Rc<crate::paint::context::PaintCx>,
        clamp: Option<(u64, crate::text::clamp::ClampMeasure)>,
        positioning: Option<Rc<crate::layout::positioned::containing_block::Positioning>>,
        build: crate::layout::build_contracts::ChildBuild,
    ) -> Self {
        Self {
            paint,
            build,
            clamp,
            positioning,
            last_clamp: Default::default(),
            window,
            cx,
            vert,
            fragment: fragment.cloned(),
            last_geometry: Default::default(),
        }
    }
}

#[derive(Clone)]
pub(super) struct FragmentFlow {
    pub(super) node: FlowNode,
    pub(super) probes: Rc<FlowMeasurements>,
}

impl FragmentFlow {
    pub(super) fn new(
        parent: Rc<FlowMeasurements>,
        probes: Rc<FlowMeasurements>,
        axis: StackAxis,
    ) -> Self {
        Self {
            node: FlowNode::new(parent, axis),
            probes,
        }
    }

    pub(super) fn publish(&self, plan: &Plan, kids: &[Kid]) {
        let mut geometry = FlowGeometry::default();
        append(&mut geometry, kids, &plan.slots);
        geometry.normalize();
        *self.node.geometry.borrow_mut() = geometry;
    }
}

fn append(geometry: &mut FlowGeometry, kids: &[Kid], slots: &[Slot]) {
    for (kid, slot) in kids.iter().zip(slots) {
        if matches!(kid.kind, super::Kind::Float { .. }) {
            geometry.floats.push(FloatGeometry {
                id: kid.node_id,
                top: slot.y,
                bottom: slot.y + slot.b,
                margins: slot.float_margins,
                first_break: (!kid.monolithic)
                    .then(|| slot.geometry.cuts.first().map(|&(a, _)| slot.y + a))
                    .flatten(),
            });
        }
        let (start, end) = if kid.monolithic && matches!(kid.kind, super::Kind::Float { .. }) {
            // Float margins stay with an unbreakable box during balancing.
            (
                slot.y - slot.float_margins[0],
                slot.y + slot.b + slot.float_margins[1],
            )
        } else {
            (slot.y, slot.y + slot.b)
        };
        if kid.monolithic {
            geometry.solid.push((start, end));
        } else if let Some(nest) = &kid.nest {
            // Nested slots already use the host's coordinates.
            append(geometry, &nest.kids, &slot.kids);
        } else {
            geometry.append(&slot.geometry, slot.y);
        }
        geometry.cuts.push((start, start));
        geometry.cuts.push((end, end));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::float::band_flow::{Edge, Kind};

    #[test]
    fn unused_block_size_is_fragmentable_unless_the_box_is_monolithic() {
        let mut kid = Kid {
            node_id: 0,
            pagination_offset: 0.0,
            kind: Kind::Float {
                side: -1,
                clear: None,
                shrink: false,
                letter: false,
                own_layout: false,
            },
            monolithic: false,
            clear: None,
            margin: [Edge::Px(0.); 4],
            build: Rc::new(|_, _, _, _, _| panic!("geometry does not rebuild children")),
            nest: None,
            anon: false,
            lead: None,
            lead_base: None,
            margin_offset: 0.,
            start_open: true,
            shape: None,
        };
        let slot = Slot {
            b: 200.,
            ..Default::default()
        };
        let mut geometry = FlowGeometry::default();
        append(
            &mut geometry,
            std::slice::from_ref(&kid),
            std::slice::from_ref(&slot),
        );
        geometry.normalize();
        assert!(geometry.solid.is_empty());
        assert_eq!(geometry.cuts, vec![(200., 200.)]);
        kid.monolithic = true;
        let mut geometry = FlowGeometry::default();
        append(&mut geometry, std::slice::from_ref(&kid), &[slot]);
        assert_eq!(geometry.solid, vec![(0., 200.)]);
        let slot = Slot {
            y: 40.,
            b: 20.,
            float_margins: [40., 40.],
            ..Default::default()
        };
        let mut geometry = FlowGeometry::default();
        append(&mut geometry, std::slice::from_ref(&kid), &[slot]);
        geometry.normalize();
        assert_eq!(geometry.solid, vec![(0., 100.)]);
        assert_eq!(geometry.cuts, vec![(100., 100.)]);
        assert_eq!(geometry.floats[0].first_break, None);
        kid.monolithic = false;
        let slot = Slot {
            y: 50.,
            b: 100.,
            geometry: FlowGeometry::lines([70., 30.]),
            ..Default::default()
        };
        let mut geometry = FlowGeometry::default();
        append(&mut geometry, &[kid], &[slot]);
        assert_eq!(geometry.floats[0].first_break, Some(120.));
        assert_eq!(geometry.floats[0].bottom, 150.);
    }
}
