//! Обход геометрии вложенных полос с явными кадрами вместо рекурсии.
use super::super::{Kid, Slot};
use crate::layout::fragment::flow::{FloatGeometry, FlowGeometry};

struct Pending<'a> {
    kids: std::iter::Zip<std::slice::Iter<'a, Kid>, std::slice::Iter<'a, Slot>>,
    geometry: FlowGeometry,
    owner: Option<(&'a Kid, &'a Slot, f32, f32)>,
}

pub(super) fn append(geometry: &mut FlowGeometry, kids: &[Kid], slots: &[Slot]) {
    let mut pending = vec![Pending {
        kids: kids.iter().zip(slots),
        geometry: std::mem::take(geometry),
        owner: None,
    }];
    loop {
        let current = pending
            .last_mut()
            .expect("root geometry remains until return");
        let Some((kid, slot)) = current.kids.next() else {
            let Pending {
                geometry: mut child,
                owner,
                ..
            } = pending.pop().unwrap();
            let Some((kid, slot, start, end)) = owner else {
                *geometry = child;
                return;
            };
            finish(
                &mut pending.last_mut().unwrap().geometry,
                &mut child,
                kid,
                slot,
                start,
                end,
            );
            continue;
        };
        let geometry = &mut current.geometry;
        geometry
            .boxes
            .insert(kid.node_id, (slot.y, slot.y + slot.b));
        let mut child = FlowGeometry::default();
        if matches!(kid.kind, super::super::Kind::Float { .. }) {
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
        let (start, end) = if kid.monolithic && matches!(kid.kind, super::super::Kind::Float { .. })
        {
            // Float margins stay with an unbreakable box during balancing.
            (
                slot.y - slot.float_margins[0],
                slot.y + slot.b + slot.float_margins[1],
            )
        } else {
            (slot.y, slot.y + slot.b)
        };
        if kid.monolithic {
            if !matches!(kid.kind, super::super::Kind::Float { .. }) {
                geometry.solid.push((start, end));
            }
        } else if let Some(nest) = &kid.nest {
            pending.push(Pending {
                kids: nest.kids.iter().zip(slot.kids.iter()),
                geometry: child,
                owner: Some((kid, slot, start, end)),
            });
            continue;
        } else {
            child.append(&slot.geometry, slot.y);
        }
        finish(geometry, &mut child, kid, slot, start, end);
    }
}

fn finish(
    geometry: &mut FlowGeometry,
    child: &mut FlowGeometry,
    kid: &Kid,
    slot: &Slot,
    start: f32,
    end: f32,
) {
    prefix(child, kid.flow_key);
    if matches!(kid.kind, super::super::Kind::Float { .. }) {
        child.float_leads.extend(
            child
                .line_pushes
                .iter()
                .filter(|p| (p.at - slot.y).abs() < crate::tolerance::COORD_EPS)
                .cloned()
                .map(|p| (kid.node_id, p)),
        );
    }
    geometry.append(child, 0.0);
    geometry.cuts.push((start, start));
    geometry.cuts.push((end, end));
}

pub(super) fn prefix(geometry: &mut FlowGeometry, key: usize) {
    for p in &mut geometry.inline {
        p.scope.insert(0, key);
    }
    for f in &mut geometry.float_contexts {
        f.scope.insert(0, key);
    }
}
