//! Draw the prepared root or replay the recorded frame. A host that changes
//! the placement or skips the document step supplies its final inputs here.

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
    checked: Option<super::prepared::CheckedRoot>,
    layout_inputs: gpui::RetainedLayoutInputs,
    /// No replay candidate: the tree is built at `request_layout`, where the
    /// hover the window shows is known (`Builder::build`).
    unbuilt: bool,
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
                checked: Some(prepared.checked),
                layout_inputs: prepared.layout_inputs,
                unbuilt: false,
            };
        }
        Self {
            slot,
            key,
            builder,
            tree: None,
            flow: None,
            scheduled: 0,
            replay: false,
            same_prepaint: false,
            scroll_plan: None,
            root_bars: None,
            laid_out: None,
            checked: None,
            layout_inputs: Default::default(),
            unbuilt: !candidate,
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

    /// Use the document store's identity across edits to the element tree.
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
        (self.request_viewport_layout(window, cx), ())
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
        self.builder.state.scrolling.painted(!self.replay);
        if let Some((bars, hits)) = self.root_bars.take() {
            bars.paint(&hits, window);
        }
    }
}

type RootBarsNow = (
    Rc<crate::interactive::scroll_area::RootBars>,
    Vec<crate::interactive::scrollbars::interaction::HitBar>,
);
