//! Элемент сохраняемого кадра: собирает и записывает кадр документа либо
//! повторяет записанный в прошлом кадре.
//!
//! Решение принимается на prepaint, когда известны границы и окружение окна.
//! Кандидат повтора на раскладке занимает узел того же размера, что коробка
//! области просмотра; если запись не подошла, дерево собирается здесь же и
//! раскладывается отдельным корнем в тех же границах. Коробка имеет точный
//! размер, поэтому её содержимое раскладывается так же, как в потоке.

use super::{Builder, FrameKey, Slot};
mod build_in;
mod frame_record;
use crate::layout::reuse::FlowReuse;
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
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
    same_prepaint: bool,
    /// The scroll nodes of the replayed record moved (`prepaint` to `paint`).
    scroll_plan: Option<gpui::ScrollReplay>,
    /// The root scrollport's bars and their hit areas this frame, drawn
    /// after the record (`RootBars`).
    root_bars: Option<RootBarsNow>,
    /// Дерево собрано и разложено шагом документа до кадра (`prepared`) для
    /// элемента документа под этим местом в кадре: коробка области просмотра
    /// — лист её размера.
    laid_out: Option<gpui::ElementPlace>,
}

impl Retained {
    pub(super) fn new(
        slot: Rc<Slot>,
        key: Option<FrameKey>,
        builder: Builder,
        candidate: bool,
        prepared: Option<super::prepared::Prepared>,
    ) -> Self {
        if let Some(prepared) = prepared {
            return Self {
                slot,
                key,
                builder: {
                    builder.counters.set(prepared.counters);
                    builder
                },
                tree: Some(prepared.tree),
                flow: prepared.flow,
                scheduled: prepared.scheduled,
                replay: false,
                same_prepaint: false,
                scroll_plan: None,
                root_bars: None,
                laid_out: Some(prepared.parent),
            };
        }
        let (tree, flow) = if candidate {
            (None, None)
        } else {
            slot.builds.set(slot.builds.get() + 1);
            slot.inside_draw.set(slot.inside_draw.get() + 1);
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
            same_prepaint: false,
            scroll_plan: None,
            root_bars: None,
            laid_out: None,
        }
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
        Some(super::prepared::document_id(&self.builder.store))
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
        self.check_place(window);
        let before = window.next_frame_callback_count();
        let id = match self.tree.as_mut() {
            Some(_) if self.laid_out.is_some() => {
                window.request_layout(self.viewport_style(), None, cx)
            }
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
        if let Some(root) = &self.builder.state.root_scroll {
            root.hosted.set(true);
        }
        self.prepaint_record(id, bounds, window, cx);
        // After the record: hit areas of the bars at the offset now.
        let bars = self
            .builder
            .state
            .root_scroll
            .as_ref()
            .and_then(|root| root.bars.borrow().clone());
        self.root_bars = bars.map(|bars| {
            let hits = bars.prepaint(window);
            (bars, hits)
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
        self.paint_record(id, window, cx);
        if let Some((bars, hits)) = self.root_bars.take() {
            bars.paint(&hits, window);
        }
    }
}

type RootBarsNow = (
    Rc<crate::interactive::scroll_area::RootBars>,
    Vec<crate::interactive::scrollbars::interaction::HitBar>,
);
