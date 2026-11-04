//! Элемент сохраняемого кадра: собирает и записывает кадр документа либо
//! повторяет записанный в прошлом кадре.
//!
//! Решение принимается на prepaint, когда известны границы и окружение окна.
//! Кандидат повтора на раскладке занимает узел того же размера, что коробка
//! области просмотра; если запись не подошла, дерево собирается здесь же и
//! раскладывается отдельным корнем в тех же границах. Коробка имеет точный
//! размер, поэтому её содержимое раскладывается так же, как в потоке.

use super::record::Record;
use super::window_key::WindowKey;
use super::{Builder, FrameKey, Slot};
use crate::layout::reuse::FlowReuse;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, RetainedPaint, RetainedPrepaint, Style,
    Window, px, size,
};
use std::rc::Rc;

pub(super) struct Retained {
    slot: Rc<Slot>,
    key: Option<FrameKey>,
    builder: Builder,
    /// Дерево этого кадра; `None` — кандидат повтора.
    tree: Option<AnyElement>,
    /// Повтор блоков корневого потока дерева этого кадра.
    flow: Option<Rc<FlowReuse>>,
    /// Кадры, заказанные поддеревом на этом кадре: такой кадр не устоялся.
    scheduled: usize,
    replay: bool,
}

impl Retained {
    pub(super) fn new(
        slot: Rc<Slot>,
        key: Option<FrameKey>,
        builder: Builder,
        candidate: bool,
    ) -> Self {
        let (tree, flow) = if candidate {
            (None, None)
        } else {
            slot.builds.set(slot.builds.get() + 1);
            let (tree, flow) = builder.build(true, false);
            (Some(tree), flow)
        };
        Self {
            slot,
            key,
            builder,
            tree,
            flow,
            scheduled: 0,
            replay: false,
        }
    }

    fn viewport_style(&self) -> Style {
        let (w, h) = self.builder.opts.viewport;
        Style {
            size: size(px(w).into(), px(h).into()),
            ..Style::default()
        }
    }

    /// Дерево не подошло к записи: собрать и разложить в тех же границах.
    fn build_in(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> RetainedPrepaint {
        let space = size(
            AvailableSpace::Definite(bounds.size.width),
            AvailableSpace::Definite(bounds.size.height),
        );
        let (_, prepaint) = window.record_retained_prepaint(|window| {
            // Заместители, отказанные проверкой прошлой сборки, собираются;
            // если и эта не прошла (другое место корня), то без заместителей.
            let mut allow = true;
            loop {
                self.slot.builds.set(self.slot.builds.get() + 1);
                let (mut tree, flow) = self.builder.build(allow, true);
                tree.layout_as_root(space, window, cx);
                let fits = flow.as_ref().is_none_or(|flow| flow.validate(window));
                if fits || !allow {
                    tree.prepaint_at(bounds.origin, window, cx);
                    self.tree = Some(tree);
                    self.flow = flow;
                    return;
                }
                if let Some(flow) = &flow {
                    flow.note_rebuild();
                }
                allow = false;
            }
        });
        prepaint
    }
}

impl IntoElement for Retained {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Retained {
    type RequestLayoutState = ();
    type PrepaintState = ();

    /// Ключ — записи блоков документа: они переживают правки (новый `Slot`),
    /// и состояние элемента должно пережить их тоже, чтобы видимые блоки
    /// повторялись из прошлого кадра (`Store::draw`). Запись кадра чужого
    /// `Slot` отсеивает `Record::reusable`.
    fn id(&self) -> Option<ElementId> {
        Some(ElementId::NamedInteger(
            "retained-document".into(),
            Rc::as_ptr(&self.builder.store) as usize as u64,
        ))
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let before = window.next_frame_callback_count();
        let id = match self.tree.as_mut() {
            Some(tree) => {
                let keys = self.builder.layout_keys;
                window.with_layout_keys(keys, |window| tree.request_layout(window, cx))
            }
            None => window.request_layout(self.viewport_style(), None, cx),
        };
        self.scheduled += window.next_frame_callback_count().saturating_sub(before);
        (id, ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(id) = id else { return };
        let key = WindowKey::capture(bounds, window, cx);
        let root = window.retained_root_environment();
        window.with_element_state::<Record, _>(id, |record, window| {
            let before = window.next_frame_callback_count();
            // Состояние элемента живёт кадр: без него прошлый кадр окна не
            // рисовал документ, и повтор блоков из него невозможен.
            self.builder.store.draw(record.is_some());
            let record = record.filter(|r| r.reusable(&self.slot, &self.key, &key));
            if let Some(r) = &record
                && !window.retained_replayable(&r.prepaint, &r.paint)
            {
                self.slot.unreplayable.set(true);
                *self.slot.recorded.borrow_mut() = None;
            }
            if self.tree.is_none()
                && root
                && !self.slot.unreplayable.get()
                && let Some(record) = record
            {
                self.replay = true;
                self.slot.replays.set(self.slot.replays.get() + 1);
                let prepaint = window.replay_retained_prepaint(&record.prepaint);
                return ((), Record { prepaint, ..record });
            }
            // Заместитель, оказавшийся у экрана, отменяет пропуск: кадр
            // собирается заново без заместителей (`layout::reuse`).
            if let Some(flow) = self.flow.as_ref()
                && self.tree.is_some()
                && !flow.validate(window)
            {
                flow.note_rebuild();
                self.tree = None;
            }
            let prepaint = match self.tree.as_mut() {
                Some(tree) => {
                    window
                        .record_retained_prepaint(|window| tree.prepaint(window, cx))
                        .1
                }
                None => self.build_in(bounds, window, cx),
            };
            self.scheduled += window.next_frame_callback_count().saturating_sub(before);
            let record = Record {
                slot: Rc::downgrade(&self.slot),
                key: self.key.clone(),
                window: Some(key),
                prepaint,
                paint: RetainedPaint::default(),
                root,
                settled: false,
            };
            ((), record)
        });
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(id) = id else { return };
        window.with_element_state::<Record, _>(id, |record, window| {
            let mut record = record.unwrap_or_default();
            if self.replay {
                record.paint = window.replay_retained_paint(&record.paint);
                return ((), record);
            }
            let Some(tree) = self.tree.as_mut() else {
                return ((), record);
            };
            let before = window.next_frame_callback_count();
            let (_, paint) = window.record_retained_paint(|window| tree.paint(window, cx));
            if let Some(flow) = &self.flow {
                flow.finish();
            }
            self.scheduled += window.next_frame_callback_count().saturating_sub(before);
            record.settled = record.root
                && paint.replayable()
                && self.scheduled == 0
                && self.builder.state.values.frame_ready();
            record.paint = paint;
            record.settled &= !self.slot.unreplayable.get();
            *self.slot.recorded.borrow_mut() = self.key.clone().filter(|_| record.settled);
            ((), record)
        });
    }
}
