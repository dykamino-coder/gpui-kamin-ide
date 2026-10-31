//! Обход кусков колонок: подготовка завершается до входа в потомка, чтобы её
//! крупные временные значения не накапливались на нативном стеке.

use crate::layout::fragment::types::{Frag, frag_copy};
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, Bounds, Pixels, Window};

impl ColumnStack {
    // Общая геометрия колонок и контекст GPUI для обхода фрагментов.
    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    pub(super) fn paint_frags(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        bounds: Bounds<Pixels>,
        col_w: f32,
        step: f32,
        parts: Vec<usize>,
        plan_all: Vec<Frag>,
        plan: Vec<Frag>,
    ) {
        let records = self.copy_records();
        let mut replayed = std::collections::HashMap::new();
        for f in plan {
            let geometry = (bounds, col_w, step);
            let piece = self.prepare_paint_piece(&f, &plan_all, &parts, geometry, window, cx);
            if self.replays(f.kid, &plan_all)
                && self.paint_piece_replayed(&f, &plan_all, geometry, &mut replayed, window, cx)
            {
                continue;
            }
            // Срез рисует полную копию под полосой колонки; целая коробка
            // сохраняет собственное переполнение (css-break-3 §4).
            if piece.split {
                self.paint_slice(&f, &piece, col_w, records.as_ref(), window, cx);
            } else {
                let kid = &mut self.children[f.kid];
                let Some(el) = frag_copy(&mut kid.el, &mut kid.frags, f.copy) else {
                    continue;
                };
                crate::hooks::with_copy_segment(
                    gpui::PaintSegmentId(f.col as u64),
                    records.as_ref(),
                    window,
                    |window| el.paint(window, cx),
                );
            }
        }
        crate::layout::fragment::replay::paint_late(&replayed, window);
    }
}
