//! Сбор геометрии промежутков стопки для `GapRulePainter` (css-gaps-1 §gap-multicol):
//! коробки колонок, линии с числом занятых колонок и спаннеры во всю ширину. Берётся из
//! готового плана укладки, независимо от щупов отдельных элементов.
use super::ColumnStack;
use crate::layout::fragment::types::{Frag, axis_box};
use crate::paint::gap_rules::MulticolGaps;
use crate::tolerance::COORD_EPS;
use gpui::{Bounds, Pixels};
impl ColumnStack {
    pub(super) fn collect_gap_geometry(
        &self,
        bounds: Bounds<Pixels>,
        col_w: f32,
        plan: &[Frag],
        spans: &[(usize, f32)],
    ) {
        let Some(buffer) = &self.gap_items else {
            return;
        };
        let (inline, block) = self.axis_sizes(bounds);
        let lines = self.lines_plan.borrow();
        let wrap = self.rows.is_some_and(|r| r.wrap);
        let mut used = vec![0; lines.len()];
        for f in plan.iter().filter(|f| f.h > COORD_EPS) {
            let line = if wrap { f.col / self.count } else { 0 };
            let col = if wrap { f.col % self.count } else { f.col };
            if let Some(u) = used.get_mut(line) {
                *u = (*u).max(col + 1);
            }
        }
        let count = used.iter().copied().max().unwrap_or(0).max(self.count);
        let box_at = |i, iw, b, bh| axis_box(self.axis, bounds.origin, block, i, iw, b, bh);
        let mut geometry = MulticolGaps::default();
        for c in 0..count {
            geometry
                .columns
                .push(box_at(c as f32 * (col_w + self.gap), col_w, 0.0, block));
        }
        for (l, &(y, h)) in lines.iter().enumerate() {
            let h = if l + 1 == lines.len() && self.rows.is_none_or(|r| r.cap) {
                self.rule_to.map_or(h, |end| h.max(end - y))
            } else {
                h
            };
            geometry.rows.push((box_at(0.0, inline, y, h), used[l]));
        }
        for &(kid, y) in spans {
            geometry
                .spanners
                .push(box_at(0.0, inline, y, self.children[kid].h));
        }
        buffer.borrow_mut().multicol = Some(geometry);
    }
}
