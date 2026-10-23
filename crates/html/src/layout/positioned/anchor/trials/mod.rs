//! Раздельные layout-пробы position-try; реестры prepaint видит лишь победитель.

pub(crate) mod inline;
mod run;
pub(crate) use run::run;
mod owned;
mod signature;
mod sizing;
mod snapshot;
pub(super) mod tactics;
#[cfg(test)]
mod tests;

use super::{AnchorPlan, Placement, PlacementPlans};
use crate::document::state::ValueCx;
use crate::hooks::{Hooks, IntoHooked};
use crate::style::computed::Computed;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window};
use signature::Signature;
use snapshot::Accepted;
pub(crate) use snapshot::apply_style;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct Chosen {
    pub(super) index: usize,
    accepted: Option<owned::Owned>,
    signature: Option<Signature>,
}

pub(crate) struct Options {
    key: u64,
    order: u8,
    vertical: bool,
    remembered: bool,
    signature: Signature,
    pub(crate) styles: Vec<(usize, Computed)>,
}

impl Options {
    pub(crate) fn new(base: &Computed, inherited: &Computed, vcx: ValueCx<'_>) -> Self {
        let signature = Signature::of(base, vcx);
        let previous = vcx
            .doc
            .anchor_frame
            .chosen
            .borrow()
            .get(&base.anchor_key)
            .filter(|p| p.signature.as_ref() == Some(&signature) && p.accepted.is_some())
            .cloned();
        let remembered = previous.is_some();
        let mut current = base.clone();
        let index = previous.as_ref().map_or(0, |p| p.index);
        if let Some(p) = previous
            && let Some(accepted) = p.accepted
        {
            accepted.apply(&mut current, vcx);
        }
        let mut styles = vec![(index, current)];
        for k in 0..=base.position_try_fallbacks.len() {
            if k == index {
                continue;
            }
            let mut c = base.clone();
            if k != 0 {
                let fb = &base.position_try_fallbacks[k - 1];
                if let Some(name) = &fb.name {
                    let Some(rule) = vcx.doc.try_rule(name) else {
                        continue;
                    };
                    super::resolve::apply_try_rule(&mut c, &rule, vcx);
                }
                if let Some(area) = fb.area {
                    c.position_area = Some(area);
                }
                let resolved = crate::style::cascade::inherit::inherit(inherited, &c, vcx);
                Accepted::of(&resolved).apply(&mut c);
                tactics::apply(&mut c, inherited, fb.tactics, vcx);
            }
            styles.push((k, c));
        }
        Self {
            key: base.anchor_key,
            order: base.position_try_order,
            vertical: inherited.wm().vertical(),
            remembered,
            signature,
            styles,
        }
    }

    /// Первый проход — current styles; при переполнении перебор остальных.
    /// Если не подошёл ни один, сохраняются current styles, не база.
    pub(crate) fn choose(
        &self,
        probes: &[Rc<Probe>],
        window: &mut Window,
        vcx: ValueCx<'_>,
    ) -> usize {
        let places: Vec<_> = probes.iter().map(|p| p.placement(window, vcx)).collect();
        self.choose_placements(&places)
    }

    fn choose_placements(&self, places: &[Option<Placement>]) -> usize {
        if self.remembered && places[0].is_some_and(|p| !p.overflow) {
            return 0;
        }
        let start = usize::from(self.remembered);
        let mut fit: Vec<_> = (start..places.len())
            .filter(|i| places[*i].is_some_and(|p| !p.overflow))
            .collect();
        let metric = |i: usize| {
            let p = places[i].unwrap();
            match self.order {
                1 => p.imcb.0,
                2 => p.imcb.1,
                3 if self.vertical => p.imcb.0,
                3 => p.imcb.1,
                4 if self.vertical => p.imcb.1,
                4 => p.imcb.0,
                _ => 0.,
            }
        };
        if self.order != 0 {
            fit.sort_by(|a, b| metric(*b).total_cmp(&metric(*a)));
        }
        fit.first().copied().unwrap_or(0)
    }

    pub(crate) fn record(&self, chosen: usize, vcx: ValueCx<'_>) {
        let (index, style) = &self.styles[chosen];
        vcx.doc.anchor_frame.chosen.borrow_mut().insert(
            self.key,
            Chosen {
                index: *index,
                accepted: Some(owned::Owned::of(style, vcx)),
                signature: Some(self.signature.clone()),
            },
        );
    }
}

#[derive(Default)]
pub(crate) struct Probe {
    layout: Cell<Option<LayoutId>>,
    plan: RefCell<Option<AnchorPlan>>,
    pub(crate) spot: crate::layout::positioned::containing_block::SpotCell,
}

impl Probe {
    pub(in crate::layout::positioned::anchor) fn wrap(
        self: &Rc<Self>,
        child: AnyElement,
        plan: AnchorPlan,
        visibility: u8,
        key: u64,
    ) -> AnyElement {
        *self.plan.borrow_mut() = Some(plan);
        Measured {
            child,
            probe: self.clone(),
            visibility,
            key,
        }
        .hooked()
    }

    fn placement(&self, window: &mut Window, vcx: ValueCx<'_>) -> Option<Placement> {
        let id = self.layout.get()?;
        let own = Bounds {
            origin: window.layout_origin_unrounded(id),
            size: window.layout_size_unrounded(id),
        };
        let own = crate::layout::positioned::containing_block::placed_bounds(own, self.spot.get());
        Some(self.plan.borrow().as_ref()?.compute(own, window, vcx))
    }
}

struct Measured {
    child: AnyElement,
    probe: Rc<Probe>,
    visibility: u8,
    key: u64,
}

impl Hooks for Measured {
    type Layout = LayoutId;
    type Prepaint = bool;
    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, LayoutId) {
        let id = self.child.request_layout(window, cx);
        self.probe.layout.set(Some(id));
        (id, id)
    }
    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let plan = self
            .probe
            .plan
            .borrow_mut()
            .take()
            .expect("selected position-try plan");
        let values = plan.values.clone();
        let plans = PlacementPlans {
            plans: vec![plan],
            indices: vec![0],
            order: 0,
            key: self.key,
            visibility: self.visibility,
            values,
        };
        super::place::place_one(&mut self.child, &plans, bounds, *id, window, cx)
    }
    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        hidden: &mut bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        if !*hidden {
            self.child.paint(window, cx);
        }
    }
}
