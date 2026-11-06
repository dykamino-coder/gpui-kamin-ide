//! Корневой поток с повтором: заместители и записи его блочных детей.

use super::placeholder::{Placeholder, Slot};
use super::plain::edges_free;
use super::recorder::{Pending, Recorder};
use super::{Counters, FlowKey, FlowReuse, Root};
use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::AnyElement;
use std::rc::Rc;

mod mark;
mod replay;
use mark::{Mark, mark_of};

/// Поток `body`, занятый повтором в этой сборке.
pub(crate) struct FlowCx {
    reuse: Rc<FlowReuse>,
    key: Rc<FlowKey>,
    /// Уже собранный ребёнок потока содержит флоат: он может вдаваться в
    /// следующих детей, их раскладка тогда зависит не только от ключа.
    floats: std::cell::Cell<bool>,
    /// Ключ равен ключу прошлой сборки: записывать детей.
    settled: bool,
    /// Ключ отличается от прошлого только шириной потока (`FlowKey::
    /// same_except_width`) слева направо: записывать и замещать детей,
    /// чья раскладка от ширины не зависит (`width`).
    resized: bool,
    /// Первый и последний ребёнок потока замещаемы: базовые линии потока
    /// никто не читает (`edges_free`).
    edges: bool,
    /// Ключ прошлой сборки тот же вне ширин: записывать детей для повтора
    /// с их поддеревом раскладки (`replay`).
    settled_outside_widths: bool,
    /// Прошлая сборка тоже была устоявшейся: дальние блоки уже записаны для
    /// пропуска, собираются лишь ближние и изменённые.
    warm: bool,
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
        let last = if self.again {
            self.store.prev.borrow().clone()
        } else {
            let last = self.store.last.replace(Some(key.clone()));
            self.store.prev.replace(last.clone());
            last
        };
        let (settled, resized, settled_outside_widths) = match last {
            Some(last) => (
                last.same(&key),
                last.same_except_width(&key),
                last.same_outside_widths(&key),
            ),
            None => (false, false, false),
        };
        let wm = inherited.wm();
        let resized = !settled && resized && !wm.rtl() && !wm.vertical();
        let warm = if self.again {
            // Записи уже сверены первой сборкой кадра.
            self.store.warm.get()
        } else {
            if resized {
                // Записи, чья раскладка от ширины не зависит, годны и при новой.
                self.store.keep_width_independent(&key);
            } else if !settled {
                // Записи другого ключа не подойдут, а держат стили кадра записи.
                self.store.reset();
            }
            if !settled_outside_widths {
                self.store.reset_replays();
            }
            let warm = self.store.settled.replace(settled) && settled;
            self.store.warm.set(warm);
            warm
        };
        self.resized.set(resized);
        let edges = match self.root {
            Root::Body { .. } => edges_free(key.html.as_ref()) && edges_free(key.body.as_ref()),
            Root::TopLevel => false,
        };
        Some(FlowCx {
            reuse: self.clone(),
            key,
            settled,
            resized,
            edges,
            settled_outside_widths,
            warm,
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
        if !self.reuse.allow || edge && !self.edges || e.node_id == 0 || self.floats.get() {
            return None;
        }
        if self.reuse.store.refused.borrow().contains(&e.node_id) {
            return None;
        }
        let record = self.reuse.store.same_tree(e)?;

        let width = || {
            self.resized && record.fits_width(&self.key) && record.key.same_except_width(&self.key)
        };
        let same = record.key.same(&self.key);
        // Растянутый блок при другой ширине — та же раскладка в коробке
        // другой ширины: лист по потоку, краска записи не годна.
        let stretched = !same && record.stretch;
        // Ближний блок без свежего повтора почти наверняка попадёт на экран,
        // а отказ заместителя на экране собирает кадр второй раз. Растянутый
        // блок не повторяется никогда, поэтому заменяется только далёкий.
        let paintable = record.far || (!stretched && record.current(&self.reuse));
        let fits = (same || width()) && paintable && Counters::read(&opts.cx) == record.start;
        if !fits {
            return None;
        }
        record.end.write(&opts.cx);
        let mut next = self.reuse.next.take();
        next.insert(e.node_id, record.clone());
        self.reuse.next.replace(next);
        let slot = Rc::new(Slot::new(record, stretched));
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

    /// Обернуть только что собранный элемент ребёнка `e` записями, если его
    /// сборка дала ровно один элемент: записью пропуска и повтора на месте
    /// (`recorder`), если сборка инертна, и записью повтора с поддеревом
    /// раскладки (`replay`), если она может пригодиться.
    pub(crate) fn record(
        &self,
        e: &Element,
        edge: bool,
        mark: Mark,
        opts: &RenderOpts,
        out: &mut Vec<AnyElement>,
    ) {
        let now = mark_of(opts, out.len());
        let single = !self.floats.get()
            && e.node_id != 0
            && now.out == mark.out + 1
            && now.outlines == mark.outlines
            && now.coordinates == mark.coordinates
            && now.late == mark.late
            && out.last().is_some_and(|el| el.paint_metadata().is_none());
        if !single {
            return;
        }
        if (self.settled || self.resized) && (!edge || self.edges) && mark.inert.of(e) {
            self.record_skip(e, &mark, now.start, opts, out);
        }
        self.record_replay(e, mark, now.start, opts, out);
    }

    /// Запись пропуска и повтора на месте (`recorder`) инертного ребёнка `e`.
    fn record_skip(
        &self,
        e: &Element,
        mark: &Mark,
        end: Counters,
        opts: &RenderOpts,
        out: &mut Vec<AnyElement>,
    ) {
        // Запись при другой ширине потока годна лишь независимому от неё блоку.
        let width = super::width::independent(e, opts.cx.vcx());
        let flow = self.key.flow_width();
        if !self.settled && !self.reuse.width_recordable(e, width, flow) {
            return;
        }
        let Some(mut child) = out.pop() else { return };
        // Обёртка таблицы у начального края: лист снимается сквозь неё.
        let unpainted = child
            .downcast_mut::<crate::hooks::Hooked<crate::layout::table::StartWrapper>>()
            .is_some();
        let pending = Rc::new(Pending::new(
            self.key.clone(),
            mark.start,
            end,
            width,
            self.settled,
            unpainted,
            super::width::tracks_flow(e),
        ));
        let source = self.reuse.shells.source(e);
        super::push(&self.reuse.pending, (pending.clone(), source));
        out.push(Recorder::new(child, pending).hooked());
    }
}
