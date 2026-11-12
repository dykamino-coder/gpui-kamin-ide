//! Запись пропуска и повтора на месте (`recorder`) инертного ребёнка
//! корневого потока.

use super::super::Record;
use super::super::recorder::{Making, Recorder};
use super::{Counters, FlowCx, Mark};
use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use gpui::AnyElement;

impl FlowCx {
    /// Обернуть собранный элемент инертного ребёнка `e` записью пропуска,
    /// если её раскладка годна при ключе потока этой сборки; ответ — номер
    /// записи среди записей сборки (`FlowReuse::pending`), если обёрнут.
    pub(super) fn record_skip(
        &self,
        e: &Element,
        mark: &Mark,
        end: Counters,
        opts: &RenderOpts,
        out: &mut Vec<AnyElement>,
    ) -> Option<usize> {
        // Запись при другой ширине потока годна лишь независимому от неё блоку.
        let width = super::super::width::independent(e, opts.cx.vcx());
        let flow = self.key.flow_width();
        if !self.settled && !self.reuse.width_recordable(e, width, flow) {
            return None;
        }
        let mut child = out.pop()?;
        // Обёртка таблицы у начального края: лист снимается сквозь неё.
        let unpainted = child
            .downcast_mut::<crate::hooks::Hooked<crate::layout::table::StartWrapper>>()
            .is_some();
        // The one allocation of a block being recorded: its record.
        let pending = std::rc::Rc::new(Record::new(
            self.reuse.shells.source(e),
            self.key.clone(),
            (mark.start, end),
            Making::new(
                width,
                self.settled,
                unpainted,
                super::super::width::tracks_flow(e),
            ),
        ));
        let at = self.reuse.pending.borrow().len();
        super::super::push(&self.reuse.pending, pending);
        out.push(Recorder::new(child, (self.reuse.clone(), at)).hooked());
        Some(at)
    }
}
