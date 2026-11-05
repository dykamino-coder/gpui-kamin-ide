//! Перенос clamp-геометрии из временных движков в координаты плана полос.

use super::{BandFlow, Kid, Plan, Probe};
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
    let mut out = MeasuredClamp::default();
    let mut pending = vec![plan.slots.iter()];
    while let Some(iter) = pending.last_mut() {
        let Some(s) = iter.next() else {
            pending.pop();
            continue;
        };
        let (x, y) = match vert {
            None => (s.x, s.y),
            Some(true) => (plan.height - s.y - s.b, s.x),
            Some(false) => (s.y, if up { plan.width - s.x - s.i } else { s.x }),
        };
        out.append(&s.clamp, point(px(x), px(y)));
        pending.push(s.kids.iter());
    }
    *output.borrow_mut() = out;
}

pub(super) async fn nest(
    kid: &Kid,
    cb: f32,
    avail: f32,
    h: f32,
    p: &mut Probe<'_>,
    stack: &mut gpui::PaintStack,
) -> (MeasuredClamp, super::prepared::Prepared) {
    if p.clamp.is_none() {
        return Default::default();
    }
    let build = kid.build.clone();
    let build: super::Build =
        Rc::new(move |cb, avail, shapes, _, inline| build(cb, avail, shapes, Some(h), inline));
    super::probe_of(kid.kind, &build, cb, avail, None, None, p, stack).await;
    (p.last_clamp.clone(), p.last_prepared.clone())
}
