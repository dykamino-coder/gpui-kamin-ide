//! Запись пропуска и повтора на месте (`recorder`) инертного ребёнка
//! корневого потока.

use super::super::Record;
use super::super::recorder::{Making, Pending};
use super::{Counters, FlowCx, Mark};
use crate::dom::Element;
use crate::layout::options::RenderOpts;
use gpui::AnyElement;

impl FlowCx {
    /// Начать запись пропуска собранного элемента инертного ребёнка `e`,
    /// если её раскладка годна при ключе потока этой сборки: номер записи
    /// среди записей сборки. Обёртку ставит `record_far` — одну на блок.
    pub(super) fn record_skip(
        &self,
        e: &Element,
        mark: &Mark,
        end: Counters,
        opts: &RenderOpts,
        out: &mut [AnyElement],
    ) -> Option<usize> {
        // Запись при другой ширине потока годна лишь независимому от неё блоку.
        let width = super::super::width::independent(e, opts.cx.vcx());
        let flow = self.key.flow_width();
        if !self.settled && !self.reuse.width_recordable(e, width, flow) {
            return None;
        }
        let child = out.last_mut()?;
        // Обёртка таблицы у начального края: лист снимается сквозь неё.
        let unpainted = child
            .downcast_mut::<crate::hooks::Hooked<crate::layout::table::StartWrapper>>()
            .is_some();
        // The one allocation of a block being recorded: its record. What the
        // frame measures of it goes with the build's list (`Pending`).
        let record = std::rc::Rc::new(Record::new(
            self.reuse.shells.source(e),
            self.key.clone(),
            (mark.start, end),
        ));
        let making = Making::new(
            width,
            self.settled,
            (unpainted, super::super::width::tracks_flow(e)),
            self.reuse.ink.open(),
        );
        let at = self.reuse.pending.borrow().len();
        super::super::push(&self.reuse.pending, Pending { record, making });
        Some(at)
    }
}
