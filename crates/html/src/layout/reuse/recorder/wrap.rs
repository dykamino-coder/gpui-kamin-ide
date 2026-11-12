//! Обёртка записи в дереве GPUI: снимает лист, начало коробки, инертность
//! prepaint и чернила краски блока прямо в его запись (`Making`).

use super::{Output, far};
use crate::hooks::Wrap;
use crate::layout::reuse::width::reusable_range;
use crate::layout::reuse::{FlowReuse, Record, lines::with_record, scaled};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, ScaledPixels, Window, point, px, size};
use std::rc::Rc;

/// The wrap knows the record it makes by number: `FlowReuse` owns it, so a
/// record that failed can go before the frame tree does.
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

    /// Number of the record among the build's records (`FlowReuse::pending`).
    pub(in crate::layout::reuse) fn at(&self) -> usize {
        self.at
    }

    /// The record being made; `None` once the frame finished it.
    fn record(&self) -> Option<Rc<Record>> {
        self.reuse.pending.borrow().get(self.at).cloned()
    }
}

impl Record {
    /// The record in the making already stands for its block at every wider
    /// flow: the block prepainted and painted inertly, and its layout holds
    /// there (`width::reusable_range`: a fixed width, or text on one line).
    /// Asked after the block's paint by its far recorder (`far::FarRecorder`).
    pub(in crate::layout::reuse) fn holds_wider_flows(&self) -> bool {
        let m = &self.making;
        let range = || {
            reusable_range(
                m.width,
                m.border.get(),
                self.key.flow_width(),
                m.lines.slack(),
            )
        };
        m.quiet.get()
            && m.painted.get()
            && !m.sink.tainted()
            && range().is_some_and(|(_, hi)| hi == f32::INFINITY)
    }
}

impl Wrap for Recorder {
    fn paint_boundary(&self) -> bool {
        false
    }

    /// The wrap is transparent to dispatch: it has no key context, focus or
    /// handlers of its own, and its child's node hangs off the node above.
    fn dispatch_node(&self) -> bool {
        false
    }

    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let id = self.child.request_layout(window, cx);
        if let Some(record) = self.record() {
            record.making.layout.set(Some(id));
        }
        id
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some(record) = self.record() else {
            self.child.prepaint(window, cx);
            return;
        };
        let making = &record.making;
        making.border.set(bounds.size.width.as_f32());
        let unpainted = making.unpainted;
        let callbacks = window.next_frame_callback_count();
        let root = window.retained_root_environment()
            && window.element_offset() == point(px(0.0), px(0.0));
        let (_, range) = window.record_retained_prepaint(|window| {
            with_record(&record, window, |window| self.child.prepaint(window, cx));
        });
        let hits = range
            .quiet(window)
            .filter(|_| root && callbacks == window.next_frame_callback_count());
        let (Some(id), Some(hits)) = (making.layout.get(), hits) else {
            return;
        };
        let scale = window.scale_factor();
        making.hits.set(hits.map(|b| Bounds {
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
            making.border.set(snapshot.width());
        }
        let origin = scaled(window.layout_origin_unrounded(id), scale);
        making.quiet.set(snapshot.is_some());
        record.snapshot.set(snapshot);
        making.origin.set(Some(origin));
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
            making.output.replace(Some(Box::new(Output {
                prepaint: range,
                paint: None,
            })));
        }
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some(record) = self.record() else {
            self.child.paint(window, cx);
            return;
        };
        // The record is its own ink sink: opening it allocates nothing.
        // Вывод вынимается на время краски: ячейка не остаётся занятой.
        let Some(mut output) = record.making.output.take() else {
            window.with_ink_sink(record.clone(), |window| self.child.paint(window, cx));
            record.making.painted.set(true);
            return;
        };
        let mask = window.content_mask().bounds;
        // Строки блока рисуются в конце кадра стопки (`paint::coordinator`).
        let ((_, paint), deferred) = crate::hooks::paint_capturing(window, |window| {
            window.with_ink_sink(record.clone(), |window| {
                window.record_retained_paint(|window| self.child.paint(window, cx))
            })
        });
        output.paint = Some((paint, deferred, mask));
        record.making.output.replace(Some(output));
        record.making.painted.set(true);
    }
}
