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
    /// Ключ отличается от прошлого только шириной потока (`FlowKey::
    /// same_except_width`) слева направо: записывать и замещать детей,
    /// чья раскладка от ширины не зависит (`width`).
    resized: bool,
    /// Первый и последний ребёнок потока замещаемы: базовые линии потока
    /// никто не читает (`edges_free`).
    edges: bool,
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
        let (settled, resized) = match last {
            Some(last) => (last.same(&key), last.same_except_width(&key)),
            None => (false, false),
        };
        let wm = inherited.wm();
        let resized = !settled && resized && !wm.rtl() && !wm.vertical();
        if resized {
            // Записи, чья раскладка от ширины не зависит, годны и при новой.
            self.store.records.borrow_mut().retain(|_, r| r.resizable);
        } else if !settled {
            // Записи другого ключа не подойдут, а держат стили кадра записи.
            self.store.records.take();
        }
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
            floats: std::cell::Cell::new(false),
        })
    }
}

/// Обёртка `html`/`body` (или её отсутствие) не читает базовых линий и
/// строк крайних детей потока: блочная коробка без псевдоэлементов первой
/// строки и буквы, без обрезки текстовых краёв (css-inline-3 §5) и без
/// обрезки содержимого, под которой повтор краски был бы под другой маской.
fn edges_free(e: Option<&Element>) -> bool {
    use crate::style::computed::{Display, Overflow};
    let Some(e) = e else { return true };
    let style = e.style.used();
    let b = &style.reset_box_layout;
    let visible = |o: Option<Overflow>| matches!(o, None | Some(Overflow::Visible));
    e.first_line.is_none()
        && e.first_letter.is_none()
        && matches!(b.display, None | Some(Display::Block))
        && visible(b.overflow_x)
        && visible(b.overflow_y)
        && !style.reset_text_box.text_box_trim_start
        && !style.reset_text_box.text_box_trim_end
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
        let record = self.reuse.store.records.borrow().get(&e.node_id).cloned()?;
        let width = || self.resized && record.resizable && record.key.same_except_width(&self.key);
        // Ближний блок без свежего повтора почти наверняка попадёт на экран.
        let paintable = record.far || record.current(&self.reuse);
        let fits = (record.key.same(&self.key) || width())
            && paintable
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
        let inert = (self.settled || self.resized)
            && (!edge || self.edges)
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
        // Запись при другой ширине потока годна лишь независимому от неё блоку.
        let resizable = super::width::independent(e, opts.cx.vcx());
        if !self.settled && !resizable {
            return;
        }
        let Some(child) = out.pop() else { return };
        let pending = Rc::new(Pending::new(
            self.key.clone(),
            mark.start,
            now.start,
            resizable,
        ));
        super::push(&self.reuse.pending, (pending.clone(), Box::new(e.clone())));
        out.push(Recorder::new(child, pending).hooked());
    }
}
