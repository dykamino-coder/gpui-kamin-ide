//! Защита рекурсивных проходов представления; исходный DOM принадлежит revision,
//! как и соответствие его элементов узлам дерева коробок (`Element::node_id`).

use super::DocumentValues;
use crate::dom::input::Admitted;
use crate::dom::outcome::{FailureScope, TraversalKind, TraversalLimit};
use markup5ever_rcdom::Handle;
use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

#[cfg(feature = "heap-report")]
mod heap;

/// Предел построения коробок: 2 × 1026, как nsCSSFrameConstructor Gecko.
/// Исходный DOM этим пределом не ограничен.
pub const MAX_DEPTH: usize = crate::budget::DocumentLimits::BOX_DEPTH;

#[derive(Default)]
pub struct TraversalState {
    source: RefCell<Option<Admitted>>,
    bindings: OnceCell<Rc<RefCell<super::bindings::BindingStore>>>,
    recording: RefCell<super::bindings::BindingRecorder>,
    first_limit: Cell<Option<TraversalLimit>>,
    embedded: Cell<bool>,
    /// A computation read the viewport size (`values::viewport_reads`).
    pub viewport_read: Cell<bool>,
    /// The viewport-dependent media queries of the build being recorded
    /// (`style::css::reads::record`).
    pub media_record: RefCell<Option<crate::style::css::reads::MediaReads>>,
    /// Equal paints of this document's boxes, shared (SharedPaints).
    pub paints: crate::style::decoration_paint::SharedPaints,
}

impl TraversalState {
    pub fn retain(&self, source: Admitted) {
        self.source.replace(Some(source));
    }

    /// Borrow the admitted source of the current revision.
    pub fn source(&self) -> std::cell::Ref<'_, Admitted> {
        std::cell::Ref::map(self.source.borrow(), |s| {
            s.as_ref().expect("document owns its source DOM")
        })
    }

    /// Забрать исходный DOM: следующая ревизия собирается из него же
    /// (`retain` возвращает его этой ревизии, если сборка отвергнута).
    pub fn take_source(&self) -> Option<Admitted> {
        self.source.take()
    }

    /// A full conversion has no retained owners; failed prior drafts are aborted.
    pub fn begin_full_bindings(&self) {
        self.recording.replace(Default::default());
    }

    fn binding_owner(&self) -> &Rc<RefCell<super::bindings::BindingStore>> {
        self.bindings.get_or_init(Default::default)
    }

    /// Share only current identity metadata, never the previous DocumentValues.
    pub fn binding_store(&self) -> Rc<RefCell<super::bindings::BindingStore>> {
        self.binding_owner().clone()
    }

    /// Attach before a new ValueCx reads source identities. Recording stays local
    /// to this traversal; only the common prepared publisher may change the map.
    pub fn attach_bindings(
        &self,
        store: Rc<RefCell<super::bindings::BindingStore>>,
    ) -> Result<(), Rc<RefCell<super::bindings::BindingStore>>> {
        self.bindings.set(store)
    }

    /// Exclusive current map for the common prepared memo/binding publisher.
    pub fn bindings_mut(&self) -> std::cell::RefMut<'_, super::bindings::BindingStore> {
        self.binding_owner().borrow_mut()
    }

    /// Start/end one output owner's direct source observations.
    pub fn begin_bindings(&self, owner: markup5ever_rcdom::NodeKey) {
        self.recording.borrow_mut().begin_owner(owner);
    }

    pub fn end_bindings(
        &self,
        owner: markup5ever_rcdom::NodeKey,
        admission: &crate::budget::admission::Admission,
    ) -> Result<std::rc::Rc<super::bindings::BindingSegment>, crate::budget::admission::Error> {
        self.recording.borrow_mut().finish_owner(owner, admission)
    }

    /// A capability already finalized by the ordinary walk; no descendant scan.
    pub fn binding_segment(
        &self,
        owner: markup5ever_rcdom::NodeKey,
    ) -> Option<std::rc::Rc<super::bindings::BindingSegment>> {
        self.recording.borrow().segment(owner)
    }

    pub fn record_text(&self, handle: &Handle) -> Option<crate::dom::TextId> {
        self.recording.borrow_mut().text(handle)
    }

    pub fn read_source_text(&self, handle: &Handle) {
        self.recording.borrow_mut().read(handle);
    }

    pub fn read_text(&self, id: crate::dom::TextId) {
        self.recording
            .borrow_mut()
            .read_text(id, &self.binding_owner().borrow());
    }

    pub fn record_element(&self, handle: &Handle) {
        self.recording.borrow_mut().element(handle);
    }

    /// One common map, not copied descendant lists or an ordinal source index.
    pub fn bindings(&self) -> std::cell::Ref<'_, super::bindings::BindingStore> {
        self.binding_owner().borrow()
    }

    /// Initial conversion publishes only after every fallible conversion step.
    /// Retained conversion instead passes its update to the common publisher.
    pub fn publish_bindings(
        &self,
        admission: &crate::budget::admission::Admission,
    ) -> Result<(), crate::budget::admission::Error> {
        let recording = self.take_binding_recording(admission)?;
        let mut replacement = super::bindings::BindingStore::default();
        let mut stage = replacement.stage();
        stage.recorded(recording)?;
        let update = stage.into_update();
        replacement
            .prepare(update, [], admission)?
            .expect("fresh source bindings")
            .publish();
        self.binding_owner().replace(replacement);
        Ok(())
    }

    pub fn take_binding_recording(
        &self,
        admission: &crate::budget::admission::Admission,
    ) -> Result<super::bindings::BindingRecording, crate::budget::admission::Error> {
        std::mem::take(&mut *self.recording.borrow_mut()).into_recording(admission)
    }

    pub fn set_embedded(&self) {
        self.embedded.set(true);
    }

    /// Область, к которой относятся наблюдения этого документа.
    pub fn scope(&self) -> FailureScope {
        if self.embedded.get() {
            FailureScope::EmbeddedDocument
        } else {
            FailureScope::Document
        }
    }

    /// Запомнить первый предел ревизии и вернуть это наблюдение для журнала.
    pub fn limit(
        &self,
        traversal: TraversalKind,
        limit: usize,
        attempted: usize,
    ) -> TraversalLimit {
        let event = TraversalLimit {
            scope: self.scope(),
            traversal,
            limit,
            attempted,
        };
        if self.first_limit.get().is_none() {
            self.first_limit.set(Some(event));
        }
        event
    }
}

impl DocumentValues {
    pub fn input_context(&self) -> Option<crate::dom::input::Context> {
        self.traversal
            .source
            .borrow()
            .as_ref()
            .map(Admitted::context)
    }

    /// Проход представления упёрся в предел: статус ревизии и запись журнала.
    pub fn traversal_limit_reached(
        &self,
        traversal: TraversalKind,
        limit: usize,
        attempted: usize,
    ) {
        let event = self.traversal.limit(traversal, limit, attempted);
        self.record_diagnostic(event.into());
    }

    /// Полный исходный DOM, включая детей за пределом представления и templates.
    pub fn source_dom(&self) -> Option<Handle> {
        self.traversal
            .source
            .borrow()
            .as_ref()
            .map(|s| s.dom.document.clone())
    }

    /// Номер ключа, с которого продолжит повторный разбор этого документа.
    pub fn next_node_key(&self) -> Option<u64> {
        self.traversal
            .source
            .borrow()
            .as_ref()
            .map(|s| s.dom.next_key())
    }

    /// Первый достигнутый предел этой revision, сохраняемый между кадрами.
    pub fn traversal_limit(&self) -> Option<TraversalLimit> {
        self.traversal.first_limit.get()
    }
}

#[cfg(any(test, feature = "engine-test-support"))]
pub mod inventory_cases {
    use super::*;

    pub fn first_limit_is_retained() {
        let state = TraversalState::default();
        state.limit(TraversalKind::BoxBuild, MAX_DEPTH, MAX_DEPTH + 1);
        let first = state.first_limit.get().unwrap();
        state.limit(TraversalKind::BoxBuild, MAX_DEPTH, MAX_DEPTH + 2);
        assert_eq!(state.first_limit.get(), Some(first));
        assert_eq!((first.limit, first.attempted), (MAX_DEPTH, MAX_DEPTH + 1));
    }
}
