//! The band host's side of its kept result: which builds may use it, the
//! journal codes of a miss, and the numbers a plan is kept by.

use super::super::{BandFlow, Plan, Slot};
use super::HostSlot;
use std::rc::Rc;

impl BandFlow {
    /// Keep and reuse the host's plans between frames in `slot`.
    pub(crate) fn kept(mut self, slot: Option<HostSlot>) -> Self {
        self.slot = slot.map(Rc::new);
        self
    }

    /// The kept result, unless this build measures under a scope the key
    /// does not cover (fragmentation, line clamping and its positioning).
    pub(in super::super) fn host_slot(&self) -> Option<Rc<HostSlot>> {
        let plain = self.fragment.is_none() && self.clamp.is_none() && self.positioning.is_none();
        if !plain && self.slot.is_some() {
            crate::why::note("band_slot.none", || "fragmented or clamped".into());
        }
        self.slot.clone().filter(|_| plain)
    }
}

/// The `why` code of a miss of a host's or a child's record.
pub(super) fn code(kid: bool) -> &'static str {
    if kid {
        "band_kid.miss"
    } else {
        "band_slot.miss"
    }
}

/// `plan` without its prepared elements, which belong to its frame.
pub(super) fn numbers(plan: &Plan) -> Plan {
    fn strip(slots: &[Slot]) -> Vec<Slot> {
        slots
            .iter()
            .map(|s| Slot {
                prepared: Default::default(),
                kids: strip(&s.kids),
                ..s.clone()
            })
            .collect()
    }
    Plan {
        width: plan.width,
        height: plan.height,
        slots: strip(&plan.slots),
        measured_extent: plan.measured_extent.clone(),
    }
}