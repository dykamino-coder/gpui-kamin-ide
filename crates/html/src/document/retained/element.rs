//! Draw the prepared root or replay the recorded frame. A host that changes
//! the placement or skips the document step supplies its final inputs here.

use super::record::{Inputs, Record};
use super::window_key::WindowKey;
use super::{Builder, FrameKey, Slot};
mod build_in;
use crate::layout::reuse::FlowReuse;
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, RetainedPaint, Window,
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
    /// Дерево собрано и разложено шагом документа до кадра (`prepared`) для
    /// элемента документа под этим местом в кадре: коробка области просмотра
    /// — лист её размера.
    laid_out: Option<gpui::ElementPlace>,
    checked: Option<super::prepared::CheckedRoot>,
    layout_inputs: gpui::RetainedLayoutInputs,
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
                laid_out: Some(prepared.parent),
                checked: Some(prepared.checked),
                layout_inputs: prepared.layout_inputs,
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
            laid_out: None,
            checked: None,
            layout_inputs: Default::default(),
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
        let root = window.retained_root_environment();
        window.with_element_state::<Record, _>(id, |record, window| {
            let before = window.next_frame_callback_count();
            let environment = self
                .key
                .as_ref()
                .map(|_| WindowKey::capture(bounds, window, cx));
            let mut state = record.unwrap_or_default();
            let epoch = state.begin(&self.builder.store, window);
            let candidate = self.tree.is_none()
                && state.matches(&self.slot, self.key.as_ref(), environment.as_ref(), window);
            if candidate && !window.retained_replayable(&state.prepaint, &state.paint) {
                self.slot.unreplayable.set(true);
                self.slot.recorded.take();
            }
            if root
                && !self.slot.unreplayable.get()
                && candidate
                && self.builder.repaint_boxes(window, cx)
            {
                if let Some(inputs) = &state.inputs {
                    window.replay_layout_inputs(&inputs.layout);
                }
                self.replay = true;
                self.builder.store.forget_replays();
                self.slot.replays.set(self.slot.replays.get() + 1);
                state.prepaint = window.replay_retained_prepaint(&state.prepaint);
                return ((), state);
            }
            if let Some(checked) = self.checked.take() {
                if !checked.holds(epoch, environment.as_ref(), window)
                    || !self.layout_inputs.scroll_unchanged()
                {
                    self.tree = None;
                    self.flow = None;
                }
            } else if let Some(flow) = &self.flow
                && !flow.validate(window)
            {
                flow.note_rebuild();
                self.tree = None;
                self.flow = None;
            }
            let prepaint = match self.tree.as_mut() {
                // Разложено корнем до кадра: на место, как в `build_in`.
                Some(tree) if self.laid_out.is_some() => {
                    window
                        .record_retained_prepaint(|window| {
                            tree.prepaint_at(bounds.origin, window, cx)
                        })
                        .1
                }
                Some(tree) => {
                    window
                        .record_retained_prepaint(|window| tree.prepaint(window, cx))
                        .1
                }
                None => self.build_in(bounds, window, cx),
            };
            self.scheduled += window.next_frame_callback_count().saturating_sub(before);
            state.inputs = self.key.clone().zip(environment).map(|(key, env)| {
                Inputs::new(key, env, window, std::mem::take(&mut self.layout_inputs))
            });
            state.prepaint = prepaint;
            state.paint = RetainedPaint::default();
            state.root = root;
            ((), state)
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
                record.painted();
                record.paint = window.replay_retained_paint(&record.paint);
                if let Some(inputs) = &record.inputs {
                    inputs.replayed(window);
                }
                return ((), record);
            }
            let Some(tree) = self.tree.as_mut() else {
                return ((), record);
            };
            let before = window.next_frame_callback_count();
            let paint = self.builder.record_paint(window, |w| tree.paint(w, cx));
            if let Some(flow) = &self.flow {
                flow.finish();
            }
            self.scheduled += window.next_frame_callback_count().saturating_sub(before);
            let settled = record.root
                && !self.slot.unreplayable.get()
                && paint.replayable()
                && self.scheduled == 0
                && self.builder.state.values.frame_ready();
            record.paint = paint;
            record.painted();
            record.publish(&self.slot, settled);
            ((), record)
        });
    }
}
