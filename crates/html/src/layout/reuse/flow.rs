//! Корневой поток с повтором: заместители и записи его блочных детей.

use super::placeholder::{Placeholder, Slot};
use super::recorder::{Pending, Recorder};
use super::replay::{PendingReplay, Replay, ReplayRecorder, ReplaySlot};
use super::{Counters, FlowKey, FlowReuse, Root};
use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::AnyElement;
use std::rc::Rc;

mod mark;
use mark::{Mark, mark_of};

/// Поток `body`, занятый повтором в этой сборке.
pub(crate) struct FlowCx {
    reuse: Rc<FlowReuse>,
    key: Rc<FlowKey>,
    /// Уже собранный ребёнок потока содержит флоат: он может вдаваться в
    /// следующих детей, их раскладка тогда зависит не только от ключа.
    floats: std::cell::Cell<bool>,
    /// Ключ равен ключу прошлой сборки: записывать детей для пропуска.
    settled: bool,
    /// Ключ прошлой сборки тот же вне ширин: записывать детей для повтора.
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
        let last = self.store.last.replace(Some(key.clone()));
        let (settled, settled_outside_widths) = match last {
            Some(last) => (last.same(&key), last.same_outside_widths(&key)),
            None => (false, false),
        };
        let warm = self.store.settled.replace(settled) && settled;
        if !settled {
            // Записи другого ключа не подойдут, а держат стили кадра записи.
            self.store.records.take();
        }
        if !settled_outside_widths {
            self.store.replays.take();
            self.store.banned.take();
            self.store.reads_widths.take();
        }
        Some(FlowCx {
            reuse: self.clone(),
            key,
            settled,
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

    /// Повтор ребёнка `e` на экране, если его запись подходит (`replay`).
    pub(crate) fn replay(&self, e: &Element, opts: &RenderOpts) -> Option<AnyElement> {
        if !self.reuse.allow
            || e.node_id == 0
            || self.floats.get()
            || self.reuse.store.banned(e.node_id)
        {
            return None;
        }
        let record = self.reuse.store.replays.borrow().get(&e.node_id).cloned()?;
        if !record.fits(&self.key, Counters::read(&opts.cx), e) {
            return None;
        }
        record.end.write(&opts.cx);
        let mut next = self.reuse.next_replays.take();
        next.insert(e.node_id, record.clone());
        self.reuse.next_replays.replace(next);
        let slot = Rc::new(ReplaySlot::new(record));
        super::push(&self.reuse.replay_slots, slot.clone());
        Some(Replay::new(slot).hooked())
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

    /// Состояние до сборки ребёнка `e`; с записью чтений ширин, если его
    /// запись повтора может пригодиться (`replay_worth_recording`).
    pub(crate) fn mark(&self, e: &Element, opts: &RenderOpts, out: usize) -> Mark {
        let mut mark = mark_of(opts, out);
        if self.replay_worth_recording(e) {
            mark.widths = Some(Rc::default());
        }
        mark
    }

    /// Запись повтора ребёнка `e` может подойти следующей сборке и окупиться.
    /// Она стоит обёртки, копии элемента, снятия раскладки, prepaint, краски и
    /// заданий строк, а остаётся только у блока возле экрана. Годится она
    /// потоку того же ключа вне ширин, а блок, заполняющий ширину
    /// (`plain::fills_width`) или прочитавший в прошлой записи ширину снаружи,
    /// — ещё и тех же ширин: пока ширины меняются от сборки к сборке, такой
    /// блок не пишется. Первая устоявшаяся сборка
    /// собирает весь поток; инертные блоки (`plain::plain`) она пишет для
    /// пропуска, а для повтора их пишет следующая, где дальние уже пропущены
    /// и собираются лишь ближние и изменённые. Так Blink
    /// `ShouldCreateSubsequence` кэширует подпоследовательность только там,
    /// где повтор вероятен.
    fn replay_worth_recording(&self, e: &Element) -> bool {
        self.settled_outside_widths
            && !self.floats.get()
            && match (self.settled, self.warm) {
                (true, true) => true,
                (true, false) => !super::plain::plain(e),
                (false, _) => {
                    !super::plain::fills_width(e) && !self.reuse.store.reads_widths(e.node_id)
                }
            }
    }

    /// Обернуть только что собранный элемент ребёнка `e` записью, если его
    /// сборка инертна: для пропуска вне экрана и (снаружи) для повтора.
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
        // One copy of the block's element serves both records: a block ends
        // up near the screen (replay) or far from it (skip), and the copy is
        // all either keeps of the build input.
        let copy = std::cell::OnceCell::new();
        let element = || copy.get_or_init(|| Rc::new(e.clone())).clone();
        if self.settled
            && !edge
            && super::plain::plain(e)
            && let Some(child) = out.pop()
        {
            let pending = Rc::new(Pending::new(self.key.clone(), mark.start, now.start));
            super::push(&self.reuse.pending, (pending.clone(), element()));
            out.push(Recorder::new(child, pending).hooked());
        }
        if let Some(widths) = mark.widths
            && !self.reuse.store.banned(e.node_id)
            && super::plain::replayable(e, opts.cx.vcx())
            && let Some(child) = out.pop()
        {
            let pending = Rc::new(PendingReplay::new(
                self.key.clone(),
                mark.start,
                now.start,
                widths,
            ));
            super::push(&self.reuse.pending_replays, (pending.clone(), element()));
            out.push(ReplayRecorder::new(child, pending).hooked());
        }
    }
}
