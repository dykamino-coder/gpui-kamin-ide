//! Боковые таблицы одного разбора документа, ключ — узел DOM: декларативные shadow
//! root-ы, назначения слотов, правила `::slotted` и метки `:has()` — плюс состояние
//! документа, которое читает и пишет обход.
//!
//! У `RcDom` из html5ever нет данных на узел (Blink хранит ShadowRoot элемента
//! именно там), поэтому разбор строит эти таблицы до обхода стилей и явно передаёт
//! их ему. Ключ держит свой `Handle`: пока жива таблица, называющая узел, узел не
//! может быть освобождён, а его адрес — повторно занят другим узлом; таблица
//! умирает вместе с разбором.

use crate::dom::shadow::{Shadow, SlotInfo};
use crate::style::css::Rule;
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// Identity of a DOM node: pointer equality on a kept-alive `Handle`.
#[derive(Clone)]
pub struct HandleKey(Handle);

impl HandleKey {
    pub fn of(handle: &Handle) -> Self {
        Self(handle.clone())
    }

    /// The node.
    pub fn handle(&self) -> &Handle {
        &self.0
    }
}

impl PartialEq for HandleKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for HandleKey {}

impl Hash for HandleKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(Rc::as_ptr(&self.0), state);
    }
}

/// The storage one `:has()` mark is charged for.
const MARK_BYTES: usize = 128;

/// The `:has()` marks of one marking of a DOM (`TreeIndex::take_has`).
#[derive(Default)]
pub struct HasMarks {
    marks: HashMap<HandleKey, Vec<u64>>,
    leases: crate::budget::admission::Buffer<Option<crate::budget::LiveReservation>>,
}

/// Everything the style walk reads about a node besides the node itself.
pub struct TreeIndex {
    pub(crate) style_sharing: crate::style::cascade::sharing::StyleSharing,
    pub matching: Rc<crate::style::select::cache::MatchCaches>,
    /// `:has()` marks: hashes of the arguments this node anchors a match for.
    pub has_marks: HashMap<HandleKey, Vec<u64>>,
    has_leases: crate::budget::admission::Buffer<Option<crate::budget::LiveReservation>>,
    /// Shadow tree of a host.
    pub shadows: HashMap<HandleKey, Rc<Shadow>>,
    /// Assignment of a `<slot>`.
    pub slots: HashMap<HandleKey, Rc<SlotInfo>>,
    /// `::slotted()` rules that match a flattened assigned element.
    pub slotted: HashMap<HandleKey, Rc<Vec<Rule>>>,
    /// The document being parsed: its quirks mode, `@property` registry and
    /// root metrics, which the walk reads and writes.
    pub doc: Rc<crate::values::DocumentValues>,
    /// XHTML host-language rules for attribute matching in this parse.
    pub xhtml: bool,
}

impl TreeIndex {
    pub fn admission(&self) -> crate::budget::admission::Admission {
        self.doc
            .budget
            .borrow()
            .clone()
            .map(crate::budget::admission::Admission::new)
            .unwrap_or_default()
    }

    pub fn new(doc: Rc<crate::values::DocumentValues>, xhtml: bool) -> Self {
        TreeIndex {
            style_sharing: Default::default(),
            matching: Rc::new(crate::style::select::cache::MatchCaches::new(doc.clone())),
            has_marks: Default::default(),
            has_leases: Default::default(),
            shadows: Default::default(),
            slots: Default::default(),
            slotted: Default::default(),
            doc,
            xhtml,
        }
    }

    pub fn insert_has_mark(
        &mut self,
        handle: &Handle,
        id: u64,
    ) -> Result<(), crate::budget::admission::Error> {
        use crate::budget::admission::Error;
        let admission = self.admission();
        let lease = admission.storage(MARK_BYTES)?;
        self.has_marks
            .try_reserve(1)
            .map_err(|_| Error::Allocation)?;
        let marks = self.has_marks.entry(HandleKey::of(handle)).or_default();
        marks.try_reserve(1).map_err(|_| Error::Allocation)?;
        self.has_leases.push(lease, &admission)?;
        marks.push(id);
        Ok(())
    }

    /// The `:has()` marks of this index with their storage leases, for an
    /// index of the same DOM whose `:has()` arguments read the same values
    /// (`put_has`).
    pub fn take_has(&mut self) -> HasMarks {
        let marks = std::mem::take(&mut self.has_marks);
        let mut leases = std::mem::take(&mut self.has_leases);
        // Kept marks hold one lease for all of them instead of one per mark,
        // which a marking needs while it grows; if the budget refuses the
        // joint lease, the marks keep their own.
        let admission = self.admission();
        if let Some(bytes) = leases.len().checked_mul(MARK_BYTES)
            && let Ok(lease) = admission.storage(bytes)
        {
            let mut one = crate::budget::admission::Buffer::default();
            if one.push(lease, &admission).is_ok() {
                leases = one;
            }
        }
        HasMarks { marks, leases }
    }

    /// Marks taken from an index of the same DOM (`take_has`).
    pub fn put_has(&mut self, has: HasMarks) {
        self.has_marks = has.marks;
        self.has_leases = has.leases;
    }

    pub fn has_marks_of(&self, handle: &Handle) -> Vec<u64> {
        self.has_marks
            .get(&HandleKey::of(handle))
            .cloned()
            .unwrap_or_default()
    }

    pub fn shadow_of(&self, handle: &Handle) -> Option<Rc<Shadow>> {
        self.shadows.get(&HandleKey::of(handle)).cloned()
    }

    pub fn slot_of(&self, handle: &Handle) -> Option<Rc<SlotInfo>> {
        self.slots.get(&HandleKey::of(handle)).cloned()
    }

    pub fn slotted_rules_of(&self, handle: &Handle) -> Rc<Vec<Rule>> {
        self.slotted
            .get(&HandleKey::of(handle))
            .cloned()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_compare_node_identity_not_content() {
        let dom = crate::dom::input::Admitted::parse("<p></p><p></p>", false)
            .unwrap()
            .dom;
        let body = |h: &Handle| -> Vec<Handle> { h.children.borrow().clone() };
        let html = body(&dom.document).pop().unwrap();
        let ps = body(&body(&html)[1]);
        assert!(HandleKey::of(&ps[0]) == HandleKey::of(&ps[0]));
        assert!(HandleKey::of(&ps[0]) != HandleKey::of(&ps[1]));
    }
}
