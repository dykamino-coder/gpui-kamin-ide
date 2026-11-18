//! Запись пропуска в дереве GPUI: снимает лист, начало коробки, инертность
//! prepaint и чернила краски блока в меры его записи (`Making`). Фазы записи
//! (`Skip`) идут в обёртке блока: своей (`Recorder`) или в далёкой
//! (`far::FarRecorder`), если блок пишется и для неё, — одна обёртка на блок.

use super::{Output, Pending, far};
use crate::hooks::Wrap;
use crate::layout::reuse::width::reusable_range;
use crate::layout::reuse::{FlowReuse, lines::with_record, scaled};
use gpui::{
    AnyElement, App, Bounds, InkSinkList, LayoutId, Pixels, ScaledPixels, Window, point, px, size,
};
use std::rc::Rc;

/// The wrap of a block recorded for a skip only. The wrap knows the record
/// it makes by number: `FlowReuse` owns it and its measures, so a record that
/// failed can go before the frame tree does.
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
}

/// The skip recording of record `at` of `reuse` around a block's element.
pub(in crate::layout::reuse) struct Skip<'a> {
    pub(in crate::layout::reuse) reuse: &'a Rc<FlowReuse>,
    pub(in crate::layout::reuse) at: usize,
}

impl Pending {
    /// The record in the making already stands for its block at every wider
    /// flow: the block prepainted and painted inertly (its sink in `ink`),
    /// and its layout holds there (`width::reusable_range`: a fixed width, or
    /// text on one line). Asked after the block's paint by its far recorder
    /// (`far::FarRecorder`).
    pub(in crate::layout::reuse) fn holds_wider_flows(&self, ink: &InkSinkList) -> bool {
        let m = &self.making;
        let range = || {
            reusable_range(
                m.width,
                m.border.get(),
                self.record.key.flow_width(),
                m.lines.slack(),
            )
        };
        m.quiet.get()
            && m.painted.get()
            && m.ink(ink).is_some_and(|(_, tainted)| !tainted)
            && range().is_some_and(|(_, hi)| hi == f32::INFINITY)
    }
}

impl Skip<'_> {
    /// `f` of the record being made; `None` once the frame finished it. The
    /// list is borrowed only for `f`: no child runs inside.
    fn pending<R>(&self, f: impl FnOnce(&Pending) -> R) -> Option<R> {
        self.reuse.pending.borrow().get(self.at).map(f)
    }

    /// The record stands at every wider flow (`Pending::holds_wider_flows`).
    pub(in crate::layout::reuse) fn holds_wider_flows(&self) -> bool {
        self.pending(|p| p.holds_wider_flows(&self.reuse.ink))
            .unwrap_or(false)
    }

    pub(in crate::layout::reuse) fn request_layout(
        &self,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> LayoutId {
        let id = child.request_layout(window, cx);
        self.pending(|p| p.making.layout.set(Some(id)));
        id
    }

    pub(in crate::layout::reuse) fn prepaint(
        &self,
        child: &mut AnyElement,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let width = bounds.size.width.as_f32();
        let Some(unpainted) = self.pending(|p| {
            p.making.border.set(width);
            p.making.unpainted
        }) else {
            child.prepaint(window, cx);
            return;
        };
        let callbacks = window.next_frame_callback_count();
        let root = window.retained_root_environment()
            && window.element_offset() == point(px(0.0), px(0.0));
        let (_, range) = window.record_retained_prepaint(|window| {
            with_record(self.reuse, self.at, window, |window| {
                child.prepaint(window, cx)
            });
        });
        let hits = range
            .quiet(window)
            .filter(|_| root && callbacks == window.next_frame_callback_count());
        let Some((id, hits)) = self.pending(|p| p.making.layout.get()).flatten().zip(hits) else {
            return;
        };
        let scale = window.scale_factor();
        let snapshot = match unpainted {
            true => window.layout_snapshot_unpainted(id),
            false => window.layout_snapshot(id),
        };
        let origin = scaled(window.layout_origin_unrounded(id), scale);
        // Коробка далеко от экрана: блок, скорее всего, далёкий, и повтор ему
        // не нужен. Чернила шире коробки лишь лишат блок записи (`finish`).
        let mask = window.content_mask().bounds;
        let device = |b: Bounds<Pixels>| Bounds {
            origin: scaled(b.origin, scale),
            size: size(
                ScaledPixels(b.size.width.as_f32() * scale),
                ScaledPixels(b.size.height.as_f32() * scale),
            ),
        };
        let own = Bounds {
            origin: point(ScaledPixels(0.0), ScaledPixels(0.0)),
            ..device(bounds)
        };
        let near = !far(Some(own), origin, device(mask));
        self.pending(|p| {
            let making = &p.making;
            making.hits.set(hits.map(device));
            if unpainted && let Some(snapshot) = &snapshot {
                // Переполненная таблица держит свою ширину полом сама: для
                // `width::reusable_range` блок так же широк, как она.
                making.border.set(snapshot.width());
            }
            making.quiet.set(snapshot.is_some());
            p.record.snapshot.set(snapshot);
            making.origin.set(Some(origin));
            if near {
                making.output.replace(Some(Box::new(Output {
                    prepaint: range,
                    paint: None,
                })));
            }
        });
    }

    pub(in crate::layout::reuse) fn paint(
        &self,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Вывод вынимается на время краски: ячейка не остаётся занятой.
        let Some((sink, output)) = self.pending(|p| (p.making.sink, p.making.output.take())) else {
            child.paint(window, cx);
            return;
        };
        // The block's sink is a number in the build's list: opening it
        // allocates nothing.
        let ink = (&self.reuse.ink, sink);
        let output = match output {
            None => {
                window.with_ink_sink(ink, |window| child.paint(window, cx));
                None
            }
            Some(mut output) => {
                let mask = window.content_mask().bounds;
                // Строки блока рисуются в конце кадра стопки (`paint::coordinator`).
                let ((_, paint), deferred) = crate::hooks::paint_capturing(window, |window| {
                    window.with_ink_sink(ink, |window| {
                        window.record_retained_paint(|window| child.paint(window, cx))
                    })
                });
                output.paint = Some((paint, deferred, mask));
                Some(output)
            }
        };
        self.pending(|p| {
            p.making.output.replace(output);
            p.making.painted.set(true);
        });
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
        let skip = Skip {
            reuse: &self.reuse,
            at: self.at,
        };
        skip.request_layout(&mut self.child, window, cx)
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let skip = Skip {
            reuse: &self.reuse,
            at: self.at,
        };
        skip.prepaint(&mut self.child, bounds, window, cx)
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let skip = Skip {
            reuse: &self.reuse,
            at: self.at,
        };
        skip.paint(&mut self.child, window, cx)
    }
}
