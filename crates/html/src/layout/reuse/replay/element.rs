//! Элементы повтора: обёртка записи вокруг собранного блока и заместитель,
//! который повторяет его раскладку, prepaint и краску (`super`).

use super::slot::{same, within};
use super::{PendingReplay, ReplaySlot, plain_environment};
use crate::hooks::{Hooks, Wrap, paint_capturing, replay_deferred_moved};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Point, Window};
use std::rc::Rc;
/// Прозрачная обёртка блока: снимает узлы раскладки, диапазоны prepaint и
/// краски этого кадра.
pub(in crate::layout::reuse) struct ReplayRecorder {
    child: AnyElement,
    pending: Rc<PendingReplay>,
}

impl ReplayRecorder {
    pub(in crate::layout::reuse) fn new(child: AnyElement, pending: Rc<PendingReplay>) -> Self {
        // The wrapper lays out as its child: no node of its own.
        gpui::note_layoutless_element();
        Self { child, pending }
    }
}

impl Wrap for ReplayRecorder {
    fn paint_boundary(&self) -> bool {
        false
    }

    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let start = window.layout_request_position();
        let id = self.child.request_layout(window, cx);
        let layout = window.capture_layout(start, id);
        self.pending
            .capture(|c| c.layout = layout.map(|layout| (layout, id)));
        id
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let callbacks = window.next_frame_callback_count();
        let plain = plain_environment(window);
        let (_, range) = window.record_retained_prepaint(|window| {
            let _ = self.child.prepaint(window, cx);
        });
        let quiet =
            range.quiet(window).is_some() && callbacks == window.next_frame_callback_count();
        let Some(id) = self
            .pending
            .captured(|c| c.layout.as_ref().map(|(_, id)| *id))
        else {
            return;
        };
        if plain && quiet {
            let origin = window.layout_origin_unrounded(id);
            let size = window.layout_size_unrounded(id);
            let mask = window.content_mask();

            self.pending.capture(|c| {
                c.prepaint = Some((range, origin, size));
                c.mask = Some(mask);
            });
        }
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let sink = (&self.pending.sink.0, self.pending.sink.1);
        let callbacks = window.next_frame_callback_count();
        // Строки блока рисуются в конце кадра стопки (`paint::coordinator`).
        let ((_, paint), lines) = window.with_ink_sink(sink, |window| {
            paint_capturing(window, |window| {
                window.record_retained_paint(|window| self.child.paint(window, cx))
            })
        });
        let prepaint = self
            .pending
            .captured(|c| c.prepaint.as_ref().map(|(p, _, _)| p.clone()));
        let replayable = prepaint
            .as_ref()
            .is_some_and(|prepaint| window.retained_replayable(prepaint, &paint))
            && callbacks == window.next_frame_callback_count();
        if replayable {
            let movable = prepaint
                .as_ref()
                .is_some_and(|prepaint| window.retained_movable(prepaint, &paint))
                && crate::hooks::deferred_movable(&lines);
            self.pending.capture(|c| c.movable = movable);
            // A box's content keeps the measure functions of its leaves: a
            // replay laid out at new constraints measures as a build would,
            // and the frame check refuses only that replay (`ReplaySlot`).
            if self.pending.captured(|c| Some(c.key.is_none())) == Some(true) {
                self.pending.captured(|c| {
                    let (layout, _) = c.layout.as_ref()?;
                    window.keep_retained_measures(layout, false);
                    Some(())
                });
            }
            self.pending.capture(|c| {
                c.paint = Some(paint);
                c.lines = Some(lines);
            });
            let env = (window.scale_factor(), window.frame_serial());
            self.pending.capture(|c| c.env = Some(env));
        }
    }
}

/// Заместитель блока: занятые узлы раскладки, повтор prepaint и краски.
pub(in crate::layout::reuse) struct Replay {
    slot: Rc<ReplaySlot>,
}

impl Replay {
    pub(in crate::layout::reuse) fn new(slot: Rc<ReplaySlot>) -> Self {
        Self { slot }
    }
}

impl Hooks for Replay {
    fn paint_boundary(&self) -> bool {
        false
    }

    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        // Занятое поддерево записано в проходе этого кадра: его и займёт
        // следующий (`Window::request_retained_layout`).
        let claimed = self
            .slot
            .record
            .ranges(|r| window.request_retained_layout(&mut r.layout));
        match claimed.flatten() {
            Some(id) => {
                self.slot.layout.set(Some(id));
                (id, ())
            }
            // Узлы ушли: проверка кадра отвергнет повтор.
            None => (window.request_layout(gpui::Style::default(), [], cx), ()),
        }
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), window: &mut Window, _: &mut App) {
        let offset = self.slot.offset.get();
        let mask = window.content_mask();
        self.slot.record.ranges(|r| {
            // The frame check saw no clip but the frame's (`ReplaySlot`). A
            // container's clip moves with its content, the frame's stays; a
            // copy that meets another one (its container moved otherwise, an
            // ancestor's clip changed) is clipped by it, which draws what a
            // build under it draws: its ink lay inside its mask (`movable`).
            // A copy whose ink the clip cut no longer moves, nor replays
            // clipped (`ReplayRecord::replays_clipped`). A record with
            // listeners keeps the plain copy.
            let by = if r.clipped { offset } else { Point::default() };
            let expected = Bounds {
                origin: r.mask.bounds.origin + by,
                size: r.mask.bounds.size,
            };
            let cut = !same(mask.bounds, expected) && r.movable;
            if cut {
                r.movable &= within(expected, mask.bounds);
                r.clipped = true;
            }
            r.mask = mask;
            self.slot.cut.set(cut);
            r.prepaint = window.replay_retained_prepaint_clipped(&r.prepaint, offset, cut);
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
        let (offset, cut) = (self.slot.offset.get(), self.slot.cut.get());
        self.slot.record.ranges(|r| {
            // Строки вне открытого кадра рисуются на месте: в выводе блока.
            let (lines, paint) = window.record_retained_paint(|window| {
                if cut {
                    window.replay_retained_paint_clipped(&r.paint, offset);
                } else {
                    window.replay_retained_paint_moved(&r.paint, offset);
                }
                replay_deferred_moved(window, &r.lines, offset, cut)
            });
            r.paint = paint;
            r.lines = lines;
            r.origin += offset;
            r.serial = window.frame_serial();
        });
        // Not compacted: a box's content is laid out again at every relayout
        // of its container (a table's columns), and the compacted form of a
        // paragraph measure shapes its text anew at each measure.
        if self.slot.record.key.is_none() {
            self.slot
                .record
                .ranges(|r| window.keep_retained_measures(&r.layout, false));
        }
    }
}
