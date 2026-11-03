//! Корневой поток с повтором: заместители и записи его блочных детей.

use super::placeholder::{Placeholder, Slot};
use super::recorder::{Pending, Recorder};
use super::{Counters, FlowKey, FlowReuse, Root};
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
    /// Ключ равен ключу прошлой сборки: записывать детей.
    settled: bool,
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
    /// Занять поток контейнера `inherited`, если это корневой поток этого
    /// документа (`body` или начальный содержащий блок снятых обёрток) в
    /// обычной сборке: без фрагментации, обрезки строк, руби и отложенных
    /// внепоточных сборок. Ключ, отличный от прошлой сборки, сбрасывает записи.
    pub(crate) fn claim(
        self: &Rc<Self>,
        inherited: &BoxStyle,
        opts: &RenderOpts,
    ) -> Option<FlowCx> {
        let cx = &opts.cx;
        let plain = cx.contracts.document_root == Some(self.doc)
            && cx.fragmentation.lines.is_none()
            && cx.clamp.input.is_none()
            && cx.ruby.base.is_none()
            && cx.inline.trim_requests.is_empty()
            && cx.position.oof_identities.is_none();
        let target = match self.root {
            Root::Body { body, .. } => inherited.self_node == body,
            // Стиль начального содержащего блока есть только у корня.
            Root::TopLevel => inherited.reset_context.icb,
        };
        if !target || !plain || self.claimed.replace(true) {
            return None;
        }
        let (html, body) = match self.root {
            Root::Body { html, .. } => (
                match html {
                    Some(_) => Some(self.seen.0.get()?.clone()),
                    None => None,
                },
                Some(self.seen.1.get()?.clone()),
            ),
            Root::TopLevel => (None, None),
        };
        let key = FlowKey {
            root: opts.root_style(),
            html,
            body,
            constraints: cx.constraints.clone(),
            viewport: opts.viewport,
            text: opts.text.clone(),
            normal_line_height: opts.normal_line_height,
            fonts: cx.doc().values.font_generation(),
        };
        let key = Rc::new(key);
        // Первая сборка документа ничего не пишет: её кадр и так самый дорогой
        // (каскад, шейпинг), а устоявшийся поток видно только со второй сборки
        // с тем же ключом.
        let last = self.store.last.replace(Some(key.clone()));
        let settled = match last {
            Some(last) => last.same(&key),
            None => false,
        };
        if !settled {
            // Записи другого ключа не подойдут, а держат стили кадра записи.
            self.store.records.take();
        }
        Some(FlowCx {
            reuse: self.clone(),
            key,
            settled,
            floats: std::cell::Cell::new(false),
        })
    }
}
impl FlowCx {
    /// Заместитель ребёнка `e`, если его запись (блока, бывшего далеко от
    /// экрана) подходит.
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
            && crate::dom::same_tree(&record.element, e);
        if !fits {
            return None;
        }
        record.end.write(&opts.cx);
        let mut next = self.reuse.next.take();
        next.insert(e.node_id, record.clone());
        self.reuse.next.replace(next);
        let slot = Rc::new(Slot::new(record));
        super::push(&self.reuse.placeholders, slot.clone());
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
        let inert = self.settled
            && !edge
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
        let pending = Rc::new(Pending::new(self.key.clone(), mark.start, now.start));
        super::push(&self.reuse.pending, (pending.clone(), Box::new(e.clone())));
        out.push(Recorder::new(child, pending).hooked());
    }
}
