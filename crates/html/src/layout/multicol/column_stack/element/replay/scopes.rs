//! Контексты записи главной копии: куски для слотов внепоточных корней
//! (`continuation::replayed`), сегменты `LATE` для отложенных заданий кадра записи
//! (`coordinator::with_late_jobs`) и допуск строк (`line_admission`); маски
//! воспроизведения кусков, продлённые тем, что нашла запись.

use super::Routes;
use crate::hooks::BareRecords;
use crate::layout::fragment::line_admission::{self, Lines};
use crate::layout::fragment::replay_pieces::ReplayPieces;
use gpui::{Bounds, Pixels, Window};
use std::collections::HashMap;
use std::rc::Rc;

impl Routes {
    /// `record` под контекстами записи; допуск строк возвращается для масок.
    pub(super) fn record_scopes<R>(
        &self,
        records: Option<BareRecords>,
        window: &mut Window,
        record: impl FnOnce(&mut Window) -> R,
    ) -> (R, Rc<Lines>) {
        let slots = Rc::new(ReplayPieces {
            records,
            pieces: self.slots.clone(),
        });
        let late = self.late_slices();
        let lines = (self.slices.iter().copied().zip(self.frags.iter().copied())).collect();
        let lines = Lines::new(lines, self.watched.clone(), self.spill);
        let result = crate::hooks::with_replay_pieces(slots, window, |window| {
            crate::hooks::with_late_jobs(late, window, |window| {
                line_admission::with(lines.clone(), window, record)
            })
        });
        (result, lines)
    }

    /// Маски воспроизведения: допуск узлов, сегменты `LATE`, продления художников
    /// фрагмента (`extra`) и строк.
    pub(super) fn masks(
        &self,
        natives: bool,
        extra: impl Iterator<Item = (u64, Bounds<Pixels>)>,
        lines: &Lines,
    ) -> HashMap<usize, Bounds<Pixels>> {
        let mut masks = self.nodes.masks.clone();
        self.late_masks(natives, &mut masks);
        for (segment, b) in extra.chain(lines.take_extra()) {
            let m = masks.entry(segment as usize).or_insert(b);
            *m = m.union(&b);
        }
        masks
    }
}
