//! Обёртка записи в дереве GPUI: снимает лист, начало коробки, инертность
//! prepaint и чернила краски блока в его `Pending`.

use super::{Output, Pending, far};
use crate::hooks::Wrap;
use crate::layout::reuse::{FlowReuse, lines::with_sink, scaled};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, ScaledPixels, Window, point, px, size};
use std::rc::Rc;

/// The wrap knows its pending record by number: `FlowReuse` owns it, so the
/// record can go before the frame tree does.
pub(in crate::layout::reuse) struct Recorder {
    child: AnyElement,
    reuse: Rc<FlowReuse>,
    at: usize,
}

impl Recorder {
    pub(in crate::layout::reuse) fn new(
        child: AnyElement,
        (reuse, at): (Rc<FlowReuse>, usize),
    ) -> Self {
        Self { child, reuse, at }
    }

    /// `f` of the pending record; `None` once the frame finished it.
    fn with<R>(&self, f: impl FnOnce(&Pending) -> R) -> Option<R> {
        self.reuse.pending.borrow().get(self.at).map(|p| f(p))
    }
}

impl Wrap for Recorder {
    fn paint_boundary(&self) -> bool {
        false
    }

    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let id = self.child.request_layout(window, cx);
        self.with(|p| p.layout.set(Some(id)));
        id
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some((unpainted, sink)) = self.with(|p| {
            p.border.set(bounds.size.width.as_f32());
            (p.unpainted, p.lines.clone())
        }) else {
            self.child.prepaint(window, cx);
            return;
        };
        let callbacks = window.next_frame_callback_count();
        let root = window.retained_root_environment()
            && window.element_offset() == point(px(0.0), px(0.0));
        let (_, range) = window.record_retained_prepaint(|window| {
            with_sink(sink, window, |window| self.child.prepaint(window, cx));
        });
        let hits = range
            .quiet(window)
            .filter(|_| root && callbacks == window.next_frame_callback_count());
        let (Some(Some(id)), Some(hits)) = (self.with(|p| p.layout.get()), hits) else {
            return;
        };
        let pending = self.reuse.pending.borrow();
        let Some(pending) = pending.get(self.at) else {
            return;
        };
        let scale = window.scale_factor();
        pending.hits.set(hits.map(|b| Bounds {
            origin: scaled(b.origin, scale),
            size: size(
                ScaledPixels(b.size.width.as_f32() * scale),
                ScaledPixels(b.size.height.as_f32() * scale),
            ),
        }));
        let snapshot = match unpainted {
            true => window.layout_snapshot_unpainted(id),
            false => window.layout_snapshot(id),
        };
        if unpainted && let Some(snapshot) = &snapshot {
            // Переполненная таблица держит свою ширину полом сама: для
            // `width::reusable_range` блок так же широк, как она.
            pending.border.set(snapshot.width());
        }
        let origin = scaled(window.layout_origin_unrounded(id), scale);
        pending.quiet.set(snapshot.is_some());
        pending.snapshot.replace(snapshot);
        pending.origin.set(Some(origin));
        // Коробка далеко от экрана: блок, скорее всего, далёкий, и повтор ему
        // не нужен. Чернила шире коробки лишь лишат блок записи (`finish`).
        let mask = window.content_mask().bounds;
        let screen = Bounds {
            origin: scaled(mask.origin, scale),
            size: size(
                ScaledPixels(mask.size.width.as_f32() * scale),
                ScaledPixels(mask.size.height.as_f32() * scale),
            ),
        };
        let own = Bounds {
            origin: point(ScaledPixels(0.0), ScaledPixels(0.0)),
            size: size(
                ScaledPixels(bounds.size.width.as_f32() * scale),
                ScaledPixels(bounds.size.height.as_f32() * scale),
            ),
        };
        if !far(Some(own), origin, screen) {
            pending.output.replace(Some(Box::new(Output {
                prepaint: range,
                paint: None,
            })));
        }
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some((sink, output)) = self.with(|p| (p.sink.clone(), p.output.take())) else {
            self.child.paint(window, cx);
            return;
        };
        // Вывод вынимается на время краски: ячейка не остаётся занятой.
        let Some(mut output) = output else {
            window.with_ink_sink(sink, |window| self.child.paint(window, cx));
            self.with(|p| p.painted.set(true));
            return;
        };
        let mask = window.content_mask().bounds;
        // Строки блока рисуются в конце кадра стопки (`paint::coordinator`).
        let ((_, paint), deferred) = crate::hooks::paint_capturing(window, |window| {
            window.with_ink_sink(sink, |window| {
                window.record_retained_paint(|window| self.child.paint(window, cx))
            })
        });
        output.paint = Some((paint, deferred, mask));
        self.with(|p| {
            p.output.replace(Some(output));
            p.painted.set(true);
        });
    }
}
