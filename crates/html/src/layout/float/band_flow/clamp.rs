//! Перенос clamp-геометрии из временных движков в координаты плана полос.

use super::{BandFlow, Kid, Plan, Probe, Slot};
use crate::text::clamp::{ClampMeasure, MeasuredClamp};
use gpui::{point, px};
use std::{cell::RefCell, rc::Rc};

impl BandFlow {
    pub(crate) fn clamp_measure(
        mut self,
        measure: Option<(u64, ClampMeasure)>,
        positioning: Option<Rc<crate::layout::positioned::containing_block::Positioning>>,
    ) -> Self {
        self.positioning = positioning;
        self.clamp = measure;
        self
    }
}

pub(super) fn publish(output: &RefCell<MeasuredClamp>, plan: &Plan, vert: Option<bool>, up: bool) {
    fn append(out: &mut MeasuredClamp, slots: &[Slot], plan: &Plan, vert: Option<bool>, up: bool) {
        for s in slots {
            let (x, y) = match vert {
                None => (s.x, s.y),
                Some(true) => (plan.height - s.y - s.b, s.x),
                Some(false) => (s.y, if up { plan.width - s.x - s.i } else { s.x }),
            };
            out.append(&s.clamp, point(px(x), px(y)));
            append(out, &s.kids, plan, vert, up);
        }
    }
    let mut out = MeasuredClamp::default();
    append(&mut out, &plan.slots, plan, vert, up);
    *output.borrow_mut() = out;
}

pub(super) fn nest(kid: &Kid, cb: f32, avail: f32, h: f32, p: &mut Probe) -> MeasuredClamp {
    if p.clamp.is_none() {
        return MeasuredClamp::default();
    }
    let build = kid.build.clone();
    let build: super::Build =
        Rc::new(move |cb, avail, shapes, _, inline| build(cb, avail, shapes, Some(h), inline));
    super::probe_of(kid.kind, &build, cb, avail, None, None, p);
    p.last_clamp.clone()
}
