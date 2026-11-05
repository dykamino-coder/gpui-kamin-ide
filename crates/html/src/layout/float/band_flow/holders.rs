//! Держатели потомков хоста на их запланированных местах в порядке CSS 2.1
//! Appendix E. Группа paint держит свой бокс и потомков.

use super::kid::{Kid, Kind};
use super::{Layer, Slot, frame, layer};
use crate::layout::build_contracts::ChildBuild;
use crate::layout::cx::deferred::Deferred;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, point, px};
use std::cell::Cell;
use std::rc::Rc;

/// What every holder of one host shares.
pub(super) struct Placing<'a> {
    pub(super) paint: &'a Rc<Deferred>,
    pub(super) build: &'a ChildBuild,
    /// Inline size of the host's containing block.
    pub(super) cb: f32,
    /// Block size of the plan.
    pub(super) height: f32,
    pub(super) vert: Option<bool>,
    pub(super) inline_up: bool,
}

/// Holders for `kids` in paint order: block-level boxes (step 4), floats
/// (step 5), inline content (step 7), then paint groups (step 8), each at
/// its physical place relative to `origin`.
pub(super) fn holders(
    kids: &[Kid],
    slots: &[Slot],
    at: &Placing,
    origin: (f32, f32),
) -> Vec<AnyElement> {
    let mut pending = vec![Pending::new(kids, slots, origin, None)];
    loop {
        let current = pending
            .last_mut()
            .expect("root holder remains until return");
        let Some((kid, s)) = current.kids.next() else {
            let complete = pending.pop().unwrap();
            let Some((kid, s, el)) = complete.owner else {
                return complete.elements;
            };
            let (group, key) = kid.nest.as_ref().unwrap().group.as_ref().unwrap();
            let container = div()
                .relative()
                .w(px(s.i))
                .h(px(s.b))
                .child(div().absolute().left(px(0.0)).top(px(0.0)).child(el))
                .children(complete.elements);
            let parent = pending.last_mut().unwrap();
            let held = hold(group(container), physical(s, at), parent.origin);
            parent.elements.push(crate::hooks::paint_last(held, *key));
            continue;
        };
        let place = physical(s, at);
        let offset = point(
            px(place.0 - current.origin.0),
            px(place.1 - current.origin.1),
        );
        let group = kid.nest.as_ref().filter(|n| n.group.is_some());
        if group.is_none()
            && let Some(el) = s.prepared.take(Some(offset))
        {
            current.elements.push(el);
            continue;
        }
        let el = s.prepared.take(None).unwrap_or_else(|| {
            crate::work::add!(FloatChildBuilds, 1);
            frame(
                kid.kind,
                s.avail,
                (kid.build)(at.cb, s.avail, s.shapes.clone(), s.h, None),
                Rc::new(Cell::new(None)),
                at.vert.is_some(),
                at.paint,
                at.build,
            )
        });
        if let Some(nest) = group {
            pending.push(Pending::new(&nest.kids, &s.kids, place, Some((kid, s, el))));
        } else {
            current.elements.push(hold(el, place, current.origin));
        }
    }
}

struct Pending<'a> {
    kids: std::vec::IntoIter<(&'a Kid, &'a Slot)>,
    elements: Vec<AnyElement>,
    origin: (f32, f32),
    owner: Option<(&'a Kid, &'a Slot, AnyElement)>,
}

impl<'a> Pending<'a> {
    fn new(
        kids: &'a [Kid],
        slots: &'a [Slot],
        origin: (f32, f32),
        owner: Option<(&'a Kid, &'a Slot, AnyElement)>,
    ) -> Self {
        Self {
            kids: paint_order(kids, slots).into_iter(),
            elements: Vec::new(),
            origin,
            owner,
        }
    }
}

fn paint_order<'a>(kids: &'a [Kid], slots: &'a [Slot]) -> Vec<(&'a Kid, &'a Slot)> {
    let mut flat: Vec<(&Kid, &Slot)> = Vec::new();
    for which in [Layer::Block, Layer::Float, Layer::Inline, Layer::Group] {
        flatten(kids, slots, which, &mut flat);
    }
    flat.retain(|(kid, _)| !matches!(kid.kind, Kind::Strut(_)));
    flat
}

fn physical(s: &Slot, at: &Placing) -> (f32, f32) {
    // Логическое место → физическое (Blink
    // `writing_mode_converter.cc:80-110`): line-left is the bottom
    // edge in sideways-lr, so subtract the measured inline extent.
    match at.vert {
        None => (s.x, s.y),
        Some(true) => (at.height - s.y - s.b, s.x),
        Some(false) => (s.y, if at.inline_up { at.cb - s.x - s.i } else { s.x }),
    }
}

fn hold(el: AnyElement, place: (f32, f32), origin: (f32, f32)) -> AnyElement {
    let offset = point(px(place.0 - origin.0), px(place.1 - origin.1));
    div()
        .absolute()
        .left(offset.x)
        .top(offset.y)
        .child(el)
        .into_any_element()
}

/// Дети слоя `layer` и их места плоским списком в порядке краски, на любой
/// глубине (коробка `Kind::Nest` раньше своих детей). The descendants of a
/// paint group belong to the group's own holder, not to the host's layers
/// (Blink `BoxFragmentPainter::PaintFloatingChildren` skips self-painting
/// layers).
pub(super) fn flatten<'a>(
    kids: &'a [Kid],
    slots: &'a [Slot],
    which: Layer,
    out: &mut Vec<(&'a Kid, &'a Slot)>,
) {
    let mut pending = vec![kids.iter().zip(slots.iter())];
    while let Some(iter) = pending.last_mut() {
        let Some((kid, s)) = iter.next() else {
            pending.pop();
            continue;
        };
        let own = layer(kid);
        if own == which {
            out.push((kid, s));
        }
        if own == Layer::Group {
            continue;
        }
        if let Some(nest) = kid.nest.as_ref() {
            pending.push(nest.kids.iter().zip(s.kids.iter()));
        }
    }
}
