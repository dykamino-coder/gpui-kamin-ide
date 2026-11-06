//! Состояние сборки до блочного ребёнка корневого потока (`FlowCx::mark`).

use super::super::Counters;
use crate::dom::Element;
use crate::layout::options::RenderOpts;
use std::rc::Rc;

/// Состояние сборки до блочного ребёнка: его сборка должна изменить только
/// счётчики и добавить ровно один элемент.
pub(crate) struct Mark {
    pub(super) start: Counters,
    pub(super) out: usize,
    pub(super) outlines: usize,
    pub(super) coordinates: usize,
    pub(super) late: bool,
    /// Чтения ширин снаружи сборкой ребёнка (`BoxConstraints::track_widths`).
    pub(super) widths: Option<Rc<std::cell::Cell<bool>>>,
    /// Инертность ребёнка (`plain::plain`), общая для `mark` и `record`.
    pub(super) inert: super::super::plain::Inert,
}

impl Mark {
    /// Опции сборки ребёнка: с записью чтений ширин, если его записывают.
    pub(crate) fn child_opts(&self, opts: &RenderOpts) -> Option<Box<RenderOpts>> {
        let sink = self.widths.clone()?;
        Some(opts.inner(|cx| cx.constraints.track_widths(sink)))
    }
}

pub(super) fn mark_of(opts: &RenderOpts, out: usize) -> Mark {
    let paint = &opts.cx.deferred.paint;
    Mark {
        start: Counters::read(&opts.cx),
        out,
        outlines: paint.outlines.borrow().len(),
        coordinates: paint.coordinates.borrow().len(),
        late: opts.cx.position.state.late_pending(),
        widths: None,
        inert: Default::default(),
    }
}

impl super::FlowCx {
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
    /// блок не пишется. Первая устоявшаяся сборка собирает весь поток;
    /// блоки, которые она пишет для пропуска (`FlowCx::record`), для повтора
    /// пишет следующая, где дальние уже пропущены и собираются лишь ближние и
    /// изменённые. Так Blink `ShouldCreateSubsequence` кэширует
    /// подпоследовательность только там, где повтор вероятен.
    fn replay_worth_recording(&self, e: &Element) -> bool {
        self.settled_outside_widths
            && !self.floats.get()
            && (self.settled
                || !super::super::plain::fills_width(e)
                    && !self.reuse.store.reads_widths(e.node_id))
    }
}
