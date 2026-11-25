//! The band host's layout result between frames: plans by width on its box.
//!
//! A plan made in one frame is kept on the host's box record
//! (`LayoutSlot::band`) as numbers only: places, sizes, overflow and the
//! probed geometry of its children, without their prepared elements, which
//! live in the frame's element arena. A later frame whose host subtree is
//! clean (`BoxTree::build_epoch` not newer than the record) and whose outside
//! input is the same (`BandKey`) takes the plan at the same width without a
//! probe; its holders then build each child once at its place
//! (`holders::holder`). The record is the box's: freed with the box, and
//! reset by a newer epoch or another key, so it never outlives what it was
//! computed from. The host holds the box tree weakly.

use super::Plan;
use crate::document::boxes::BoxTree;
use crate::layout::reuse::{BandBoxes, BandKey};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
mod chain;
mod host;

/// Plans kept per host: the widths one layout pass measures a host at
/// (final, min/max-content probes of its parent).
const PLANS: usize = 4;

/// The kept result of a band host (`LayoutSlot::band`).
pub(crate) struct Retained {
    pub(super) key: Rc<BandKey>,
    /// `BoxTree::clock` of the publication the plans were made at.
    pub(super) at: u32,
    /// The next record of the same box (`slot_chain`).
    pub(super) other: Option<Box<Retained>>,
    intrinsic: Option<Stamped<(f32, f32)>>,
    plans: [Option<Stamped<Rc<Plan>>>; PLANS],
    next: usize,
    /// The probes of a child box (`kid_slot`), on its record in `LayoutSlot::probe`.
    probes: super::kid_slot::KidProbes,
}

impl std::fmt::Debug for Retained {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Retained")
            .field("at", &self.at)
            .field("plans", &self.plans.iter().flatten().count())
            .finish()
    }
}

#[cfg(test)]
impl Retained {
    /// An empty record of `key` made at publication `at`.
    pub(super) fn for_tests(key: Rc<BandKey>, at: u32) -> Self {
        Self {
            key,
            at,
            other: None,
            intrinsic: None,
            plans: Default::default(),
            next: 0,
            probes: Default::default(),
        }
    }
}

/// A kept value and the document frame (`Store::epoch`) that made it. A
/// frame reads only values of earlier frames: within its own frame the
/// value's prepared elements are still in the frame's caches, and a read
/// would build them again at prepaint.
#[derive(Clone)]
pub(super) struct Stamped<T> {
    pub(super) value: T,
    pub(super) frame: u64,
}

/// Where a host reads and writes its kept result.
pub(crate) struct HostSlot {
    boxes: Weak<RefCell<BoxTree>>,
    owner: BandBoxes,
    key: Rc<BandKey>,
    /// This build may read kept results (`FlowReuse::allows_kept`).
    read: bool,
    /// The document frame of this build (`Store::epoch`).
    pub(super) frame: u64,
}

impl HostSlot {
    pub(crate) fn new(
        boxes: &Rc<RefCell<BoxTree>>,
        owner: BandBoxes,
        key: BandKey,
        (read, frame): (bool, u64),
    ) -> Self {
        Self {
            boxes: Rc::downgrade(boxes),
            owner,
            key: Rc::new(key),
            read,
            frame,
        }
    }

    /// `f` over the host's valid record; `None` when the box is gone, the box
    /// tree is borrowed elsewhere, or the record is stale.
    fn valid<R>(&self, f: impl FnOnce(&Retained) -> Option<R>) -> Option<R> {
        self.read_at(self.owner, false, f)
    }

    /// `valid` for the record of the child with node `node` (`kid_slot`).
    pub(super) fn kid<R>(
        &self,
        node: u64,
        f: impl FnOnce(&super::kid_slot::KidProbes) -> Option<R>,
    ) -> Option<R> {
        let own = self.boxes.upgrade()?.try_borrow().ok()?.box_of(node)?;
        let at = BandBoxes {
            slot: own,
            epoch: own,
        };
        self.read_at(at, true, |kept| f(&kept.probes))
    }

    fn read_at<R>(
        &self,
        owner: BandBoxes,
        kid: bool,
        f: impl FnOnce(&Retained) -> Option<R>,
    ) -> Option<R> {
        if !self.read {
            crate::why::note("band_slot.miss", || "reading banned".into());
            return None;
        }
        let boxes = self.boxes.upgrade()?;
        let boxes = boxes.try_borrow().ok()?;
        let epoch = boxes.build_epoch(owner.epoch)?;
        let slot = boxes.slot(owner.slot);
        let head = slot.map(|s| if kid { &s.probe } else { &s.band });
        let Some(kept) = head.and_then(|head| chain::find(head, &self.key)) else {
            crate::why::note(host::code(kid), || "no record".into());
            return None;
        };
        let stale = match () {
            _ if epoch > kept.at || kept.at > boxes.clock() => Some(("subtree changed", "")),
            _ => kept.key.differs(&self.key).map(|what| ("outside input: ", what)),
        };
        if let Some((why, what)) = stale {
            crate::why::note(host::code(kid), || format!("{why}{what}"));
            return None;
        }
        f(kept)
    }

    /// The plan kept at width `cb`.
    pub(super) fn plan(&self, cb: f32) -> Option<Rc<Plan>> {
        let plan = self.valid(|kept| {
            let found = kept.plans.iter().flatten().find(|p| p.value.width == cb)?;
            self.earlier(found)
        });
        if plan.is_some() {
            crate::work::add!(LayoutCacheHits, 1);
        }
        plan
    }

    /// The kept min/max-content pair.
    pub(super) fn intrinsic(&self) -> Option<(f32, f32)> {
        self.valid(|kept| self.earlier(kept.intrinsic.as_ref()?))
    }

    /// The value of `kept`, if an earlier frame made it.
    pub(super) fn earlier<T: Clone>(&self, kept: &Stamped<T>) -> Option<T> {
        (kept.frame < self.frame).then(|| kept.value.clone())
    }

    /// `value` stamped with this build's frame.
    pub(super) fn stamp<T>(&self, value: T) -> Stamped<T> {
        Stamped {
            value,
            frame: self.frame,
        }
    }

    /// `f` over the record of this frame's input, made anew when the kept
    /// one is stale.
    fn write(&self, f: impl FnOnce(&mut Retained)) {
        self.write_at(self.owner, false, f);
    }

    /// `write` for the record of the child with node `node`.
    pub(super) fn write_kid(&self, node: u64, f: impl FnOnce(&mut super::kid_slot::KidProbes)) {
        let Some(own) = self
            .boxes
            .upgrade()
            .and_then(|b| b.try_borrow().ok()?.box_of(node))
        else {
            return;
        };
        let at = BandBoxes {
            slot: own,
            epoch: own,
        };
        self.write_at(at, true, |kept| f(&mut kept.probes));
    }

    fn write_at(&self, owner: BandBoxes, kid: bool, f: impl FnOnce(&mut Retained)) {
        let Some(boxes) = self.boxes.upgrade() else {
            return;
        };
        let Ok(mut boxes) = boxes.try_borrow_mut() else {
            return;
        };
        let (Some(epoch), clock) = (boxes.build_epoch(owner.epoch), boxes.clock()) else {
            return;
        };
        let Some(slot) = boxes.slot_mut(owner.slot) else {
            return;
        };
        let head = if kid { &mut slot.probe } else { &mut slot.band };
        let kept = chain::entry(head, &self.key, epoch, clock, || Retained {
            key: self.key.clone(),
            at: clock,
            other: None,
            intrinsic: None,
            plans: Default::default(),
            next: 0,
            probes: Default::default(),
        });
        f(kept);
    }

    /// Keep `plan` (made in this frame) by its numbers.
    pub(super) fn keep_plan(&self, plan: &Plan) {
        self.write(|kept| {
            if kept
                .plans
                .iter()
                .flatten()
                .any(|p| p.value.width == plan.width)
            {
                return;
            }
            kept.plans[kept.next] = Some(self.stamp(Rc::new(host::numbers(plan))));
            kept.next = (kept.next + 1) % PLANS;
        });
    }

    /// Keep the min/max-content pair of this frame.
    pub(super) fn keep_intrinsic(&self, sizes: (f32, f32)) {
        self.write(|kept| kept.intrinsic = Some(self.stamp(sizes)));
    }
}
