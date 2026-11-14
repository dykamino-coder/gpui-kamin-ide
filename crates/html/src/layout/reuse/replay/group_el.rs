//! Элементы повтора группы (`super::group`): обёртка записи вокруг каждого
//! собранного члена и заместитель члена. Первый заместитель повторяет
//! prepaint, побочные данные и краску всей группы; остальные только занимают
//! свои узлы раскладки.

use super::group::{GroupSlot, PendingGroup};
use super::ReplayRecorder;
use crate::hooks::{Hooks, Wrap, replay_deferred, replay_placed};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window};
use std::rc::Rc;

/// The record wrapper of member `index` of a group: the member's own
/// recorder, and the group's side data around the prepaint of the group.
pub(in crate::layout::reuse) struct GroupRecorder {
    inner: ReplayRecorder,
    group: Rc<PendingGroup>,
    index: usize,
}

impl GroupRecorder {
    pub(in crate::layout::reuse) fn new(
        inner: ReplayRecorder,
        group: Rc<PendingGroup>,
        index: usize,
    ) -> Self {
        Self {
            inner,
            group,
            index,
        }
    }
}

impl Wrap for GroupRecorder {
    fn paint_boundary(&self) -> bool {
        false
    }

    fn child(&mut self) -> &mut AnyElement {
        self.inner.child()
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        Wrap::request_layout(&mut self.inner, window, cx)
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let group = &self.group;
        if self.index == 0 {
            group.from.set(Some(group.effects.mark()));
        }
        Wrap::prepaint(&mut self.inner, bounds, window, cx);
        if self.index + 1 == group.len()
            && let Some(from) = group.from.get()
        {
            group
                .captured
                .replace(Some(group.effects.capture(from)));
        }
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        Wrap::paint(&mut self.inner, bounds, window, cx);
    }
}

/// The stand-in of member `index` of a replayed group.
pub(in crate::layout::reuse) struct GroupMember {
    slot: Rc<GroupSlot>,
    index: usize,
}

impl GroupMember {
    pub(in crate::layout::reuse) fn new(slot: Rc<GroupSlot>, index: usize) -> Self {
        Self { slot, index }
    }
}

impl Hooks for GroupMember {
    fn paint_boundary(&self) -> bool {
        false
    }

    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let index = self.index;
        let claimed = self
            .slot
            .record
            .ranges(|r| window.request_retained_layout(&mut r.members[index].0));
        match claimed.flatten() {
            Some(id) => {
                self.slot.layouts.borrow_mut()[index] = Some(id);
                (id, ())
            }
            // The nodes are gone: the frame check refuses the group.
            None => (window.request_layout(gpui::Style::default(), [], cx), ()),
        }
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), window: &mut Window, _: &mut App) {
        if self.index != 0 {
            return;
        }
        let by = self.slot.moved.get().map(|(_, by)| by).unwrap_or_default();
        self.slot.record.ranges(|r| {
            r.prepaint = window.replay_retained_prepaint(&r.prepaint);
            self.slot.effects.replay(&*r.effects, by);
        });
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        if self.index != 0 {
            return;
        }
        let moved = self.slot.moved.get();
        self.slot.record.ranges(|r| {
            let placed = moved.map(|(placed, _)| placed);
            // The record was made of members whose ink sinks were clean:
            // every listener and state read in it was inert off screen when
            // it was painted, and a sink open now (the table's own record)
            // allows them as it allowed the build's.
            let output = |window: &mut Window| {
                window.record_retained_paint(|window| {
                    match placed {
                        Some(placed) => replay_placed(window, &r.paint, placed),
                        None => {
                            window.replay_retained_paint(&r.paint);
                        }
                    }
                    replay_deferred(window, &r.lines, placed)
                })
            };
            let (lines, paint) = window.offscreen_inert(|window| (output(window), true));
            // The record is now of the group at its new place.
            if let Some(((offset, _), by)) = moved {
                for member in r.members.iter_mut() {
                    member.1 += by;
                }
                r.ink = r.ink.map(|ink| Bounds {
                    origin: ink.origin + offset,
                    size: ink.size,
                });
            }
            r.paint = paint;
            r.lines = lines;
            r.serial = window.frame_serial();
        });
    }
}
