//! Повтор ребёнка корневого потока с его поддеревом раскладки (`replay`):
//! поиск записи перед сборкой и обёртка записи после неё.

use super::super::replay::{PendingReplay, Replay, ReplayRecorder, ReplaySlot};
use super::{Counters, FlowCx, Mark};
use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use gpui::AnyElement;
use std::rc::Rc;

impl FlowCx {
    /// Повтор ребёнка `e` на экране, если его запись подходит (`replay`).
    /// Блок, отказанный проверкой заместителей этого кадра, собирается: его
    /// запись повтора не подошла бы на том же новом месте. Повторная сборка
    /// кадра (`FlowReuse::again`) не повторяет: она в том же проходе дерева
    /// раскладки, где первая уже заняла поддеревья записей, а занятые узлы
    /// поддерево не отдаёт (`Window::request_retained_layout`).
    pub(crate) fn replay(&self, e: &Element, opts: &RenderOpts) -> Option<AnyElement> {
        let store = &self.reuse.store;
        if !self.reuse.allow
            || self.reuse.again
            || e.node_id == 0
            || self.floats.get()
            || store.banned(e.node_id)
            || store.refused.borrow().contains(&e.node_id)
        {
            return None;
        }
        let record = store.replays.borrow().get(&e.node_id).cloned()?;
        if !record.fits(&self.key, Counters::read(&opts.cx), e) {
            return None;
        }
        record.end.write(&opts.cx);
        let mut next = self.reuse.next_replays.take();
        next.insert(e.node_id, record.clone());
        self.reuse.next_replays.replace(next);
        let slot = Rc::new(ReplaySlot::new(record));
        super::super::push(&self.reuse.replay_slots, slot.clone());
        Some(Replay::new(slot).hooked())
    }

    /// Обернуть собранный элемент ребёнка `e` записью повтора, если её
    /// стоит делать (`Mark::widths`) и поддерево повторяемо.
    pub(super) fn record_replay(
        &self,
        e: &Element,
        mark: Mark,
        end: Counters,
        opts: &RenderOpts,
        out: &mut Vec<AnyElement>,
    ) {
        let Some(widths) = mark.widths else { return };
        if self.reuse.store.banned(e.node_id) || !super::super::plain::replayable(e, opts.cx.vcx())
        {
            return;
        }
        let Some(child) = out.pop() else { return };
        let pending = Rc::new(PendingReplay::new(
            self.key.clone(),
            mark.start,
            end,
            widths,
            super::super::plain::fills_width(e),
        ));
        let source = self.reuse.shells.source(e);
        super::super::push(&self.reuse.pending_replays, (pending.clone(), source));
        out.push(ReplayRecorder::new(child, pending).hooked());
    }
}
