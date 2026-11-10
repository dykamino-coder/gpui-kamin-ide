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
    /// `widths` is the build's shared flag (`FrameFar::widths`): the child is
    /// built with the flow's own options recording into it (`FlowCx::child_opts`).
    pub(super) shared_widths: bool,
    /// Инертность ребёнка (`plain::plain`), общая для `mark` и `record`.
    pub(super) inert: super::super::plain::Inert,
}

impl super::FlowCx {
    /// Опции сборки ребёнка: с записью чтений ширин, если его записывают. A
    /// child whose replay record keeps its own flag gets its own copy; a far
    /// candidate is built with the flow's own options, which record into the
    /// build's shared flag (cleared by `mark`) until `give_back`, with no copy
    /// (`BoxConstraints::track_widths_in_place`).
    pub(crate) fn child_opts(&self, mark: &Mark, opts: &RenderOpts) -> Option<Box<RenderOpts>> {
        let sink = mark.widths.clone()?;
        if mark.shared_widths {
            opts.cx.constraints.track_widths_in_place(sink);
            return None;
        }
        Some(opts.inner(|cx| cx.constraints.track_widths(sink)))
    }

    /// The build of the child is over: the flow's options stop recording.
    pub(crate) fn give_back(&self, mark: &Mark, opts: &RenderOpts) {
        if mark.shared_widths {
            opts.cx.constraints.untrack_widths();
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
        } else if mark.far {
            // Width reads happen during the child's build only, and
            // `record_far` reads the flag right after it.
            let widths = self.reuse.with_far(|far| far.widths.clone());
            widths.set(false);
            mark.widths = Some(widths);
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
