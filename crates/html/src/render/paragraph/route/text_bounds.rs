//! Damage envelopes of physical words. This does not provide line fragments:
//! a synchronous CSSOM reader still needs its ordinary layout flush.

use crate::hooks::{IntoHooked, Wrap};
use crate::layout::options::RenderOpts;
use crate::layout::retained::FragmentTarget;
use crate::style::computed::BoxStyle;
use gpui::{AnyElement, App, Bounds, InkSinkList, Pixels, ScaledPixels, Window};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

#[derive(Default)]
struct Pieces {
    expected: usize,
    seen: usize,
    invalid: bool,
    area: Option<Bounds<ScaledPixels>>,
}

pub(super) struct TextBounds {
    target: FragmentTarget,
    ink: Rc<InkSinkList>,
    sink: u32,
    owners: RefCell<BTreeMap<u64, Pieces>>,
}

impl TextBounds {
    pub(super) fn new(style: &BoxStyle, opts: &RenderOpts) -> Option<Rc<Self>> {
        if style.wm().vertical() || style.wm().turned() {
            return None;
        }
        let target = crate::layout::reuse::fragment_target(opts, style.self_node)?;
        let ink = Rc::new(InkSinkList::default());
        let sink = ink.open();
        Some(Rc::new(Self {
            target,
            ink,
            sink,
            owners: RefCell::default(),
        }))
    }

    /// The child is exactly the synchronous text-only div built by `choose`.
    pub(super) fn word(self: &Rc<Self>, child: AnyElement, owner: u64) -> AnyElement {
        if owner == 0 {
            return child;
        }
        self.owners.borrow_mut().entry(owner).or_default().expected += 1;
        Word {
            child,
            owner,
            record: self.clone(),
        }
        .hooked()
    }

    pub(super) fn row(self: Rc<Self>, child: AnyElement) -> AnyElement {
        Row {
            child,
            record: self,
        }
        .hooked()
    }
}

struct Word {
    child: AnyElement,
    owner: u64,
    record: Rc<TextBounds>,
}

impl Wrap for Word {
    fn dispatch_node(&self) -> bool {
        false
    }
    fn paint_boundary(&self) -> bool {
        false
    }
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let record = &self.record;
        record.ink.with(record.sink, |s| s.clear());
        window.with_ink_sink((&record.ink, record.sink), |window| {
            self.child.paint(window, cx)
        });
        let ink = record.ink.with(record.sink, |s| (!s.tainted(), s.bounds()));
        let mut owners = record.owners.borrow_mut();
        let pieces = owners.get_mut(&self.owner).unwrap();
        pieces.seen += 1;
        match ink {
            Some((true, ink)) => {
                let area = bounds.scale(window.scale_factor());
                let area = ink.map_or(area, |ink| area.union(&ink));
                pieces.area = Some(pieces.area.map_or(area, |old| old.union(&area)));
            }
            _ => pieces.invalid = true,
        }
    }
}

struct Row {
    child: AnyElement,
    record: Rc<TextBounds>,
}

impl Wrap for Row {
    fn dispatch_node(&self) -> bool {
        false
    }
    fn paint_boundary(&self) -> bool {
        false
    }
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        for pieces in self.record.owners.borrow_mut().values_mut() {
            pieces.seen = 0;
            pieces.invalid = false;
            pieces.area = None;
        }
        self.child.paint(window, cx);
        let target = &self.record.target;
        let Ok(mut boxes) = target.boxes.try_borrow_mut() else {
            return;
        };
        let Some(slot) = boxes.slot_mut(target.owner) else {
            return;
        };
        let areas = slot.text_paint.get_or_insert_with(Default::default);
        for (&node, pieces) in self.record.owners.borrow().iter() {
            let complete = !pieces.invalid && pieces.seen == pieces.expected;
            areas.add(
                target.store.frame_epoch(),
                node,
                complete.then_some(pieces.area).flatten(),
            );
        }
    }
}
