//! Геометрия разрывов измеренного плана band до того, как временные деревья истекут.

#[cfg(test)]
use super::Slot;
use super::{Kid, Plan};
use crate::layout::fragment::flow::{FlowGeometry, FlowMeasure, FlowNode};
use std::rc::Rc;

impl super::BandFlow {
    pub(crate) fn fragment_flow(mut self, node: FlowNode, probes: Rc<FlowMeasure>) -> Self {
        self.fragment = Some(FragmentFlow { node, probes });
        self
    }
}

/// Scopes every trial layout of a probe enters: the host's line-clamp
/// measure and the positioning context whose registrations a trial isolates.
#[derive(Clone)]
pub(super) struct ProbeScopes {
    pub(super) clamp: Option<(u64, crate::text::clamp::ClampMeasure)>,
    pub(super) positioning: Option<Rc<crate::layout::positioned::containing_block::Positioning>>,
}

impl super::BandFlow {
    pub(super) fn probe_scopes(&self) -> ProbeScopes {
        ProbeScopes {
            clamp: self.clamp.clone(),
            positioning: self.positioning.clone(),
        }
    }
}

impl<'a> super::Probe<'a> {
    pub(super) fn new(
        window: &'a mut gpui::Window,
        cx: &'a mut gpui::App,
        vert: bool,
        fragment: Option<&FragmentFlow>,
        paint: Rc<crate::layout::cx::deferred::Deferred>,
        scopes: ProbeScopes,
        build: crate::layout::build_contracts::ChildBuild,
    ) -> Self {
        let ProbeScopes { clamp, positioning } = scopes;
        Self {
            engine: None,
            last_prepared: Default::default(),
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
    pub(super) probes: Rc<FlowMeasure>,
}

impl FragmentFlow {
    pub(super) fn publish(&self, plan: &Plan, kids: &[Kid]) {
        let mut geometry = FlowGeometry::default();
        append(&mut geometry, kids, &plan.slots);
        prefix(&mut geometry, self.node.ordinal + 1);
        geometry.normalize();
        *self.node.geometry.borrow_mut() = geometry;
    }
}

mod geometry;
use geometry::{append, prefix};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::float::band_flow::{Edge, Kind};

    fn node() -> Kid {
        Kid {
            intrinsic: Default::default(),
            flow: None,
            watched: None,
            flow_key: 0,
            node_id: 0,
            pagination_offset: 0.0,
            float_floor: None,
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
            rtl: false,
        }
    }

    #[test]
    fn unused_block_size_is_fragmentable_unless_the_box_is_monolithic() {
        let mut kid = node();
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
        assert_eq!(geometry.floats[0].first_break, Some(0.));
        kid.monolithic = true;
        let mut geometry = FlowGeometry::default();
        append(&mut geometry, std::slice::from_ref(&kid), &[slot]);
        assert!(geometry.solid.is_empty());
        let slot = Slot {
            y: 40.,
            b: 20.,
            float_margins: [40., 40.],
            ..Default::default()
        };
        let mut geometry = FlowGeometry::default();
        append(&mut geometry, std::slice::from_ref(&kid), &[slot]);
        geometry.normalize();
        assert!(geometry.solid.is_empty());
        assert_eq!(geometry.floats[0].balance_min(), 100.);
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

    #[test]
    fn deep_slot_geometry_uses_a_small_native_stack() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut kid = node();
                kid.kind = Kind::Flow;
                let mut slot = Slot {
                    b: 20.,
                    ..Default::default()
                };
                for id in 1..=1_000 {
                    let mut parent = node();
                    parent.node_id = id;
                    parent.kind = Kind::Nest;
                    parent.nest = Some(super::super::Nest {
                        kids: vec![kid],
                        inset: [Edge::Px(0.); 4],
                        height: None,
                        width: None,
                        group: None,
                    });
                    kid = parent;
                    slot = Slot {
                        b: 20.,
                        kids: vec![slot].into(),
                        ..Default::default()
                    };
                }
                let mut geometry = FlowGeometry::default();
                append(&mut geometry, &[kid], &[slot]);
                assert_eq!(geometry.boxes.len(), 1_001);
                assert_eq!(geometry.cuts.len(), 2_002);
                assert!(geometry.boxes.values().all(|&v| v == (0., 20.)));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
