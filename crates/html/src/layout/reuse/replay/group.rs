//! Повтор группы соседних блоков одной записью: ряд таблицы.
//!
//! Ячейки ряда — соседние дети сетки таблицы: их prepaint, краска и строки
//! идут подряд, поэтому запись ряда держит по одному диапазону каждого вида
//! на весь ряд, а на ячейку — только её поддерево раскладки (несколько чисел,
//! `RetainedSubtree`) с местом и размером. Запись на ячейку стоила ключа
//! потока, записи, строк и приёмника чернил на каждую из тысяч ячеек.
//!
//! Что сборка ряда сообщает таблице в prepaint (кромки сросшихся рамок, фоны
//! ячеек — пробы `layout::probe::Sink`), запись хранит данными (`Effects`) и
//! отдаёт их приёмникам новой сборки, сдвинув вместе с рядом. Blink держит
//! результат раскладки чистого ряда (`MaySkipRowLayout`), но красит его
//! ячейки заново; здесь чистый ряд — занятые узлы и копия трёх диапазонов.

use super::slot::placement;
use super::{PendingReplay, plain_environment};
use crate::hooks::{LineCapture, Placement};
use crate::layout::reuse::{Counters, FlowKey, Source};
use gpui::{
    Bounds, ContentMask, LayoutId, Pixels, Point, RetainedPaint, RetainedPrepaint,
    RetainedSubtree, ScaledPixels, Size, Window,
};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What the prepaint of a group tells its owner besides the frame's output:
/// values pushed into the owner's buffers of this build.
pub(crate) trait Effects {
    /// The state of the buffers before the group's prepaint.
    fn mark(&self) -> [usize; 2];
    /// The values pushed since `from`, kept with the record.
    fn capture(&self, from: [usize; 2]) -> Rc<dyn Any>;
    /// Push the kept values `data` again, moved by `by`.
    fn replay(&self, data: &dyn Any, by: Point<Pixels>);
}

/// Where and how the group lay and painted in frame `serial`.
pub(super) struct GroupRanges {
    pub(super) members: Box<[(RetainedSubtree, Point<Pixels>, Size<Pixels>)]>,
    pub(super) prepaint: RetainedPrepaint,
    pub(super) paint: RetainedPaint,
    pub(super) lines: Rc<LineCapture>,
    mask: ContentMask<Pixels>,
    scale: f32,
    pub(super) serial: u64,
    pub(super) ink: Option<Bounds<ScaledPixels>>,
    pub(super) effects: Rc<dyn Any>,
}

/// The replay record of a group: its build inputs and its last frame.
pub(crate) struct GroupRecord {
    pub(in crate::layout::reuse) source: Source,
    pub(in crate::layout::reuse) flow: Rc<FlowKey>,
    /// The owner's inputs of the group's build, compared by the owner.
    pub(in crate::layout::reuse) key: Rc<dyn Any>,
    pub(in crate::layout::reuse) widths: bool,
    pub(in crate::layout::reuse) start: Counters,
    pub(in crate::layout::reuse) end: Counters,
    members: usize,
    ranges: Cell<Option<GroupRanges>>,
}

impl GroupRecord {
    pub(super) fn ranges<R>(&self, f: impl FnOnce(&mut GroupRanges) -> R) -> Option<R> {
        let mut ranges = self.ranges.take()?;
        let out = f(&mut ranges);
        self.ranges.set(Some(ranges));
        Some(out)
    }
}

/// A group record that completes after the frame's paint.
pub(in crate::layout::reuse) struct PendingGroup {
    source: Cell<Option<Source>>,
    flow: Rc<FlowKey>,
    key: Rc<dyn Any>,
    cells: Box<[Rc<PendingReplay>]>,
    pub(super) effects: Rc<dyn Effects>,
    pub(super) from: Cell<Option<[usize; 2]>>,
    pub(super) captured: RefCell<Option<Rc<dyn Any>>>,
}

impl PendingGroup {
    pub(in crate::layout::reuse) fn new(
        source: Source,
        key: Rc<dyn Any>,
        cells: Box<[Rc<PendingReplay>]>,
        effects: Rc<dyn Effects>,
    ) -> Option<Self> {
        Some(Self {
            source: Cell::new(Some(source)),
            flow: cells.first()?.key.clone(),
            key,
            cells,
            effects,
            from: Cell::new(None),
            captured: RefCell::new(None),
        })
    }

    pub(super) fn len(&self) -> usize {
        self.cells.len()
    }

    /// The finished record with its node, if every member's output is
    /// replayable and the members' ranges follow one another. A group far
    /// from the screen is kept: its output is a few culled commands.
    pub(in crate::layout::reuse) fn finish(&self) -> Option<(u64, GroupRecord)> {
        let effects = self.captured.take()?;
        let first = self.cells.first()?;
        let (start, end, widths) = (first.start, first.end, first.widths.get());
        let (scale, serial) = first.env.get()?;
        let mask = first.mask.borrow().clone()?;
        let mut members = Vec::with_capacity(self.cells.len());
        let mut lines = Vec::with_capacity(self.cells.len());
        let mut output: Option<(RetainedPrepaint, RetainedPaint)> = None;
        let mut ink: Option<Bounds<ScaledPixels>> = None;
        for cell in self.cells.iter() {
            let (list, at) = &cell.sink;
            let (bounds, tainted) = list.with(*at, |sink| (sink.bounds(), sink.tainted()))?;
            let (layout, _) = cell.layout.take()?;
            let (prepaint, origin, size) = cell.prepaint.take()?;
            let paint = cell.paint.take()?;
            let same = cell.env.get() == Some((scale, serial))
                && cell.mask.take().is_some_and(|m| m == mask);
            if tainted || !same {
                return None;
            }
            match &mut output {
                Some((p, q)) => {
                    if !(p.append(&prepaint) && q.append(&paint)) {
                        return None;
                    }
                }
                None => output = Some((prepaint, paint)),
            }
            ink = match (ink, bounds) {
                (Some(a), Some(b)) => Some(a.union(&b)),
                (a, b) => a.or(b),
            };
            members.push((layout, origin, size));
            lines.push(cell.lines.take()?);
        }
        let (prepaint, paint) = output?;
        let record = GroupRecord {
            flow: self.flow.clone(),
            key: self.key.clone(),
            widths,
            start,
            end,
            members: members.len(),
            ranges: Cell::new(Some(GroupRanges {
                members: members.into_boxed_slice(),
                prepaint,
                paint,
                lines: LineCapture::join(&lines)?,
                mask,
                scale,
                serial,
                ink,
                effects,
            })),
            source: self.source.take()?,
        };
        Some((record.source.id(), record))
    }
}

/// The place of a replayed group in a frame.
pub(in crate::layout::reuse) struct GroupSlot {
    pub(super) record: Rc<GroupRecord>,
    pub(super) layouts: RefCell<Vec<Option<LayoutId>>>,
    /// The group lies at another place than recorded: where its paint goes
    /// and by how much it moved.
    pub(super) moved: Cell<Option<(Placement, Point<Pixels>)>>,
    pub(super) effects: Rc<dyn Effects>,
}

impl GroupSlot {
    pub(in crate::layout::reuse) fn new(record: Rc<GroupRecord>, effects: Rc<dyn Effects>) -> Self {
        Self {
            layouts: RefCell::new(vec![None; record.members]),
            record,
            moved: Cell::new(None),
            effects,
        }
    }

    pub(in crate::layout::reuse) fn members(&self) -> usize {
        self.record.members
    }

    /// `ReplaySlot::valid` of a group: every member has its recorded size,
    /// and all lie at their recorded places or moved by one offset the
    /// output can be moved by (`slot::placement`).
    pub(in crate::layout::reuse) fn valid(&self, window: &mut Window) -> bool {
        let fresh = !window.layout_stale() && plain_environment(window);
        let (serial, scale, mask) = (
            window.frame_serial(),
            window.scale_factor(),
            window.content_mask(),
        );
        let ids = self.layouts.borrow();
        let moved = self.record.ranges(|r| {
            let same =
                r.serial + 1 == serial && r.lines.complete() && r.scale == scale && r.mask == mask;
            let mut by: Option<Point<Pixels>> = None;
            for (id, (_, origin, size)) in ids.iter().zip(r.members.iter()) {
                let id = (*id).filter(|_| same && fresh)?;
                let at = window.layout_origin_unrounded(id) - *origin;
                if window.layout_size_unrounded(id) != *size || by.is_some_and(|by| by != at) {
                    return None;
                }
                by = Some(at);
            }
            let by = by?;
            if by == Point::default() {
                return Some(None);
            }
            let output = (&r.prepaint, &r.paint, &*r.lines);
            placement(by, (&r.mask, r.scale), r.ink, output, window).map(|at| Some((at, by)))
        });
        let Some(moved) = moved.flatten() else {
            return false;
        };
        self.moved.set(moved);
        true
    }
}
