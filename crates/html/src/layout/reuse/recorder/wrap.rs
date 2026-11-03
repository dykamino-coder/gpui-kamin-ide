//! Обёртка записи в дереве GPUI: снимает лист, начало коробки, инертность
//! prepaint и чернила краски блока в его `Pending`.

use super::{Output, Pending, far};
use crate::hooks::Wrap;
use crate::layout::reuse::{lines::with_sink, scaled};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, ScaledPixels, Window, point, px, size};
use std::rc::Rc;

pub(in crate::layout::reuse) struct Recorder {
    child: AnyElement,
    pending: Rc<Pending>,
}

impl Recorder {
    pub(in crate::layout::reuse) fn new(child: AnyElement, pending: Rc<Pending>) -> Self {
        Self { child, pending }
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
        self.pending.layout.set(Some(id));
        id
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        self.pending.border.set(bounds.size.width.as_f32());
        let callbacks = window.next_frame_callback_count();
        let root = window.retained_root_environment()
            && window.element_offset() == point(px(0.0), px(0.0));
        let lines = self.pending.lines.clone();
        let (_, range) = window.record_retained_prepaint(|window| {
            with_sink(lines, window, |window| self.child.prepaint(window, cx));
        });
        let hits = range
            .quiet(window)
            .filter(|_| root && callbacks == window.next_frame_callback_count());
        let (Some(id), Some(hits)) = (self.pending.layout.get(), hits) else {
            return;
        };
        let scale = window.scale_factor();
        self.pending.hits.set(hits.map(|b| Bounds {
            origin: scaled(b.origin, scale),
            size: size(
                ScaledPixels(b.size.width.as_f32() * scale),
                ScaledPixels(b.size.height.as_f32() * scale),
            ),
        }));
        let snapshot = window.layout_snapshot(id);
        let origin = scaled(window.layout_origin_unrounded(id), scale);
        self.pending.quiet.set(snapshot.is_some());
        self.pending.snapshot.replace(snapshot);
        self.pending.origin.set(Some(origin));
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
            self.pending.output.replace(Some(Box::new(Output {
                prepaint: range,
                paint: None,
            })));
        }
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let sink = self.pending.sink.clone();
        // Вывод вынимается на время краски: ячейка не остаётся занятой.
        let Some(mut output) = self.pending.output.take() else {
            window.with_ink_sink(sink, |window| self.child.paint(window, cx));
            self.pending.painted.set(true);
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
        self.pending.output.replace(Some(output));
        self.pending.painted.set(true);
    }
}
