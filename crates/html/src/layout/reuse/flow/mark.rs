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
    /// The child's subtree replay record may pay off (`replay_worth_recording`).
    pub(super) replay: bool,
    /// The child may be recorded as a far block (`FlowCx::far_candidate`).
    pub(super) far: bool,
    /// `widths` is the flow's shared flag (`FlowCx::far_widths`): the child is
    /// built with the flow's shared tracked options (`ChildOpts`).
    pub(super) shared_widths: bool,
    /// Инертность ребёнка (`plain::plain`), общая для `mark` и `record`.
    pub(super) inert: super::super::plain::Inert,
}

/// Options of tracked child builds of one flow: a child whose replay record
/// keeps its own flag gets its own copy; far candidates share one copy per
/// flow with the flow's flag, which `mark` clears before each of them.
#[derive(Default)]
pub(crate) struct ChildOpts {
    shared: Option<Box<RenderOpts>>,
    own: Option<Box<RenderOpts>>,
}

impl Mark {
    /// Опции сборки ребёнка: с записью чтений ширин, если его записывают.
    pub(crate) fn child_opts<'o>(
        &self,
        opts: &'o RenderOpts,
        slot: &'o mut ChildOpts,
    ) -> &'o RenderOpts {
        let Some(sink) = self.widths.clone() else {
            return opts;
        };
        if self.shared_widths {
            slot.shared
                .get_or_insert_with(|| opts.inner(|cx| cx.constraints.track_widths(sink)))
        } else {
            slot.own
                .insert(opts.inner(|cx| cx.constraints.track_widths(sink)))
        }
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
        replay: false,
        far: false,
        shared_widths: false,
        inert: Default::default(),
    }
}

impl super::FlowCx {
    /// Состояние до сборки ребёнка `e`; с записью чтений ширин, если его
    /// запись повтора может пригодиться (`replay_worth_recording`) или он
    /// может оказаться далёким блоком (`far_worth_recording`).
    pub(crate) fn mark(&self, e: &Element, opts: &RenderOpts, out: usize) -> Mark {
        let mut mark = mark_of(opts, out);
        mark.replay = self.replay_worth_recording(e);
        mark.far = self.far_candidate(e, opts);
        if mark.replay {
            mark.widths = Some(Rc::default());
        } else if mark.far
            && let Some(widths) = &self.far_widths
        {
            // Width reads happen during the child's build only, and
            // `record_far` reads the flag right after it.
            widths.set(false);
            mark.widths = Some(widths.clone());
            mark.shared_widths = true;
        }
        mark
    }

    /// A far record of child `e` may serve a later build of the same flow
    /// outside the widths (`far`). Whether the block is far is known only
    /// after its paint; a near one costs the wrapper and a node list.
    pub(super) fn far_worth_recording(&self, e: &Element) -> bool {
        self.settled_outside_widths && !self.floats.get() && !self.reuse.store.banned(e.node_id)
    }

    /// Child `e` is wrapped for a far record after its build: worth it, not
    /// near in its last recording (`Store::far_near`), and its subtree can
    /// be laid out again from kept nodes (`far::far_replayable`). Decided
    /// before the build, so a child that cannot be far costs no width
    /// tracking (`Mark::child_opts`).
    fn far_candidate(&self, e: &Element, opts: &RenderOpts) -> bool {
        self.far_worth_recording(e)
            && !self.reuse.store.far_near(e.node_id)
            && super::super::far::far_replayable(e, opts.cx.vcx())
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
