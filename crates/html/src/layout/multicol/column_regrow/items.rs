//! Расход элементов сеток по плану стопки (`flow::blocks::grid_items`): окна копий для краски и
//! продление сетки, пока сдвинутый элемент не кончится (Blink растит внутренний блочный размер
//! сетки на `unavailable_block_size` соседа переполненного монолитом элемента).

use crate::layout::fragment::flow::blocks::{ItemFrame, ItemResume, Pushed, RowPush, resume_items};
use crate::layout::fragment::push::Growth;
use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;

impl ColumnStack {
    /// Продолжение элементов сеток ребёнка `kid` по плану; высоты линий колонок — `lines`.
    fn item_resume(&self, kid: usize, plan: &[Frag], lines: &[(f32, f32)]) -> Option<ItemResume> {
        if self.axis.is_vertical() {
            return None;
        }
        let c = &self.children[kid];
        let (r, w) = (c.regrow.as_ref()?, c.watched.as_ref()?);
        if r.laid.placements.is_empty() {
            return None;
        }
        let mut items = Vec::new();
        w.blocks()?.resumed_grid_items(&r.laid, &mut items);
        let mut frames: Vec<ItemFrame> = plan
            .iter()
            .filter(|f| f.kid == kid)
            .filter_map(|f| {
                let &(_, height) = lines.get(f.col / self.count).or(lines.last())?;
                Some(ItemFrame {
                    copy: f.copy,
                    start: f.from,
                    end: f.from + f.h,
                    edge: f.from + height - f.y,
                })
            })
            .collect();
        frames.sort_by_key(|f| f.copy);
        Some(resume_items(&items, &frames))
    }

    /// Место после последнего ряда сетки, нужное сдвинутым элементам, чтобы кончиться.
    pub(super) fn item_growths(
        &self,
        kid: usize,
        plan: &[Frag],
        lines: &[(f32, f32)],
    ) -> Vec<Growth> {
        let Some(resume) = self.item_resume(kid, plan, lines) else {
            return Vec::new();
        };
        let Some(r) = self.children[kid].regrow.as_ref() else {
            return Vec::new();
        };
        resume
            .ends
            .into_iter()
            .filter_map(|(grid, (track, end))| {
                let rows = r.laid.rows(grid)?;
                let content = rows.last()?.1 + r.laid.growth_after(grid, track);
                (end > content + COORD_EPS).then_some(Growth {
                    target: Pushed::Row(RowPush { grid, track }),
                    at: content,
                    amount: end - content,
                    forced: false,
                })
            })
            .collect()
    }

    /// Окна элементов сеток для краски копий по окончательному плану.
    pub(crate) fn assign_grid_items(&self, plan: &[Frag]) {
        let lines = self.lines_plan.borrow().clone();
        for kid in 0..self.children.len() {
            let Some(w) = self.children[kid].watched.as_ref() else {
                continue;
            };
            let windows = self
                .item_resume(kid, plan, &lines)
                .map(|r| r.windows)
                .unwrap_or_default();
            w.set_item_windows(windows);
        }
    }
}
