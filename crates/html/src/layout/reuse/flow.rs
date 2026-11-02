//! Корневой поток с повтором: заместители и записи его блочных детей.

use super::placeholder::{Placeholder, Slot};
use super::recorder::{Pending, Recorder};
use super::{Counters, FlowKey, FlowReuse};
use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::AnyElement;
use std::rc::Rc;

/// Поток `body`, занятый повтором в этой сборке.
pub(crate) struct FlowCx {
    reuse: Rc<FlowReuse>,
    key: Rc<FlowKey>,
    /// Уже собранный ребёнок потока содержит флоат: он может вдаваться в
    /// следующих детей, их раскладка тогда зависит не только от ключа.
    floats: std::cell::Cell<bool>,
}

/// Состояние сборки до блочного ребёнка: его сборка должна изменить только
/// счётчики и добавить ровно один элемент.
pub(crate) struct Mark {
    start: Counters,
    out: usize,
    outlines: usize,
    coordinates: usize,
    late: bool,
}

fn mark_of(opts: &RenderOpts, out: usize) -> Mark {
    let paint = &opts.cx.deferred.paint;
    Mark {
        start: Counters::read(&opts.cx),
        out,
        outlines: paint.outlines.borrow().len(),
        coordinates: paint.coordinates.borrow().len(),
        late: opts.cx.position.state.late_pending(),
    }
}

impl FlowReuse {
    /// Занять поток контейнера `inherited`, если это `body` этого документа в
    /// обычной сборке: без фрагментации, обрезки строк, руби и отложенных
    /// внепоточных сборок.
    pub(crate) fn claim(
        self: &Rc<Self>,
        inherited: &BoxStyle,
        opts: &RenderOpts,
    ) -> Option<FlowCx> {
        let cx = &opts.cx;
        let plain = cx.contracts.document_root == Some(self.root)
            && cx.fragmentation.lines.is_none()
            && cx.clamp.input.is_none()
            && cx.ruby.base.is_none()
            && cx.inline.trim_requests.is_empty()
            && cx.position.oof_identities.is_none();
        if inherited.self_node != self.body || !plain || self.claimed.replace(true) {
            return None;
        }
        let (html, body) = self.seen.borrow().clone();
        let key = FlowKey {
            html: html?,
            body: body?,
            constraints: cx.constraints.clone(),
            viewport: opts.viewport,
            text: opts.text.clone(),
            normal_line_height: opts.normal_line_height,
            fonts: cx.doc().values.font_generation(),
        };
        Some(FlowCx {
            reuse: self.clone(),
            key: Rc::new(key),
            floats: std::cell::Cell::new(false),
        })
    }
}

impl FlowCx {
    /// Заместитель ребёнка `e`, если его запись подходит и он был вне экрана.
    /// `edge` — первый или последний элемент потока: его базовые линии может
    /// читать контейнер, такой ребёнок всегда собирается.
    pub(crate) fn placeholder(
        &self,
        e: &Element,
        edge: bool,
        opts: &RenderOpts,
    ) -> Option<AnyElement> {
        if !self.reuse.allow || edge || e.node_id == 0 || self.floats.get() {
            return None;
        }
        let record = self.reuse.store.records.borrow().get(&e.node_id).cloned()?;
        let fits = record.key.same(&self.key)
            && Counters::read(&opts.cx) == record.start
            && record.far()
            && crate::dom::same_tree(&record.element, e);
        if !fits {
            return None;
        }
        record.end.write(&opts.cx);
        self.reuse
            .next
            .borrow_mut()
            .insert(e.node_id, record.clone());
        let slot = Rc::new(Slot::new(record));
        self.reuse.placeholders.borrow_mut().push(slot.clone());
        Some(Placeholder::new(slot).hooked())
    }

    /// Ребёнок `n` собран (или ушёл в анонимный абзац): запомнить его флоаты.
    pub(crate) fn built(&self, n: &crate::dom::Node) {
        if let crate::dom::Node::Element(e) = n
            && !self.floats.get()
            && super::plain::floats(e)
        {
            self.floats.set(true);
        }
    }

    pub(crate) fn mark(&self, opts: &RenderOpts, out: usize) -> Mark {
        mark_of(opts, out)
    }

    /// Обернуть только что собранный элемент ребёнка `e` записью, если его
    /// сборка инертна.
    pub(crate) fn record(
        &self,
        e: &Element,
        edge: bool,
        mark: Mark,
        opts: &RenderOpts,
        out: &mut Vec<AnyElement>,
    ) {
        let now = mark_of(opts, out.len());
        let inert = !edge
            && !self.floats.get()
            && e.node_id != 0
            && now.out == mark.out + 1
            && now.outlines == mark.outlines
            && now.coordinates == mark.coordinates
            && now.late == mark.late
            && out.last().is_some_and(|el| el.paint_metadata().is_none())
            && super::plain::plain(e);
        if !inert {
            return;
        }
        let Some(child) = out.pop() else { return };
        let pending = Rc::new(Pending::new(
            e.clone(),
            self.key.clone(),
            mark.start,
            now.start,
        ));
        self.reuse.pending.borrow_mut().push(pending.clone());
        out.push(Recorder::new(child, pending).hooked());
    }
}
