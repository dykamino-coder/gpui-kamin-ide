//! Геометрия разрывов измеренного плана band до того, как временные деревья истекут.

use super::{Kid, Plan, Slot};
use crate::layout::fragment::flow::{FloatGeometry, FlowGeometry, FlowMeasure, FlowNode};
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

fn append(geometry: &mut FlowGeometry, kids: &[Kid], slots: &[Slot]) {
    for (kid, slot) in kids.iter().zip(slots) {
        geometry
            .boxes
            .insert(kid.node_id, (slot.y, slot.y + slot.b));
        let mut child = FlowGeometry::default();
        if matches!(kid.kind, super::Kind::Float { .. }) {
            let bounds = FloatGeometry {
                id: kid.node_id,
                anchor: slot.y - slot.float_margins[0],
                top: slot.y,
                bottom: slot.y + slot.b,
                margins: slot.float_margins,
                first_break: (!kid.monolithic)
                    .then(|| slot.y + slot.geometry.cuts.first().map_or(0.0, |&(a, _)| a)),
            };
            geometry.floats.push(bounds);
            child
                .float_contexts
                .push(crate::layout::fragment::flow::float_resume::Snapshot {
                    scope: Vec::new(),
                    bounds,
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
            if !matches!(kid.kind, super::Kind::Float { .. }) {
                geometry.solid.push((start, end));
            }
        } else if let Some(nest) = &kid.nest {
            // Nested slots already use the host's coordinates.
            append(&mut child, &nest.kids, &slot.kids);
        } else {
            child.append(&slot.geometry, slot.y);
        }
        prefix(&mut child, kid.flow_key);
        if matches!(kid.kind, super::Kind::Float { .. }) {
            child.float_leads.extend(
                child
                    .line_pushes
                    .iter()
                    .filter(|p| (p.at - slot.y).abs() < crate::tolerance::COORD_EPS)
                    .cloned()
                    .map(|p| (kid.node_id, p)),
            );
        }
        geometry.append(&child, 0.0);
        geometry.cuts.push((start, start));
        geometry.cuts.push((end, end));
    }
}

fn prefix(geometry: &mut FlowGeometry, key: usize) {
    for p in &mut geometry.inline {
        p.scope.insert(0, key);
    }
    for f in &mut geometry.float_contexts {
        f.scope.insert(0, key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::float::band_flow::{Edge, Kind};

    #[test]
    fn unused_block_size_is_fragmentable_unless_the_box_is_monolithic() {
        let mut kid = Kid {
            flow: None,
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
}
