//! `impl Element` для `ColumnStack`: запрос раскладки, `prepaint` (укладка по реальной
//! ширине, расстановка кусков по колонкам, спаннеры во всю ширину, границы для линеек
//! промежутков) и `paint` (маски только у разрезанных детей, позиционированные
//! фрагменты вторым проходом по CSS 2.1 Appendix E). Вертикальная ось уходит в `axis`.

use crate::hooks::Hooks;
use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, Bounds, LayoutId, Pixels, Window, point, px};
mod copies;
mod native;
mod paint;
mod prepaint;

impl ColumnStack {
    /// Горизонтальная укладка в колонки. Кадры, живые во время вложенных
    /// раскладок и prepaint копий (`prepare`, `column_breaks`,
    /// `prepaint_frags`), держатся малыми: копия может сама нести
    /// многоколоночник, и его prepaint идёт внутри этого.
    #[inline(never)]
    fn prepaint_columns(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let w = f32::from(bounds.size.width);
        let col_w = ((w - self.gap * (self.count as f32 - 1.0)) / self.count as f32).max(1.0);
        self.measure_columns(col_w, window, cx);
        let planned = self.plan_columns(bounds, col_w);
        let step = col_w + self.gap;
        self.prepaint_frags(window, cx, bounds, col_w, &planned.0, step);
        self.finish_prepaint(bounds, col_w, planned, window, cx);
    }

    /// Меры детей по реальной ширине колонки (отложенная стопка и точки разрыва).
    #[inline(never)]
    fn measure_columns(&mut self, col_w: f32, window: &mut Window, cx: &mut App) {
        if let Some(recipe) = self.deferred.clone() {
            recipe.prepare(self, col_w, window, cx);
        }
        self.column_breaks(col_w, window, cx);
    }

    /// Native OOF, спаннеры и линейки — после prepaint копий.
    #[inline(never)]
    fn finish_prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        col_w: f32,
        planned: (crate::layout::fragment::plan::Plan, Vec<(usize, f32)>),
        window: &mut Window,
        cx: &mut App,
    ) {
        let (mut plan, spans) = planned;
        if self.prepaint_native(bounds, &plan, window, cx).is_err() {
            plan.failure = Some(crate::layout::fragment::token::Error::InvalidGeometry);
            plan.incomplete();
        }
        self.prepaint_spanners(bounds, col_w, plan, spans, window, cx);
    }
}

impl Hooks for ColumnStack {
    type Layout = LayoutId;
    type Prepaint = Bounds<Pixels>;

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        self.request_column_layout(window, cx)
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*state),
            size: window.layout_size_unrounded(*state),
        };
        if self.axis.is_vertical() {
            self.prepaint_axis(bounds, window, cx);
            return bounds;
        }
        self.prepaint_columns(bounds, window, cx);
        bounds
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let bounds = *prepaint;
        if self.axis.is_vertical() {
            self.paint_axis(bounds, window, cx);
            return;
        }
        if self.oof.plan.borrow().failure.is_some() || self.record_native(window, cx).is_err() {
            return;
        }
        let plan = self.plan.borrow().clone();
        let col_w = self.col_w.get();
        let step = col_w + self.gap;
        // Маска нужна ТОЛЬКО разрезанному ребёнку. У целого она обрезала бы
        // его собственное переполнение, которого коробка не прячет: отсюда
        // уходили в красное `overflow-clip-004`, `overflow-unsplittable-*`,
        // `overflowing-block-003` и родня.
        let mut parts = vec![0usize; self.children.len()];
        for f in &plan {
            parts[f.kid] += 1;
        }
        let plan_all = plan.fragments.clone();
        // CSS 2.1 Appendix E: блочные потомки потока (шаг 4) раньше
        // позиционированных (шаг 8) — фрагменты позиционированных детей
        // красятся вторым проходом, в порядке разметки.
        let plan: Vec<Frag> = plan
            .iter()
            .filter(|f| !self.children[f.kid].positioned)
            .chain(plan.iter().filter(|f| self.children[f.kid].positioned))
            .copied()
            .collect();
        self.paint_frags(window, cx, bounds, col_w, step, parts, plan_all, plan);
        let spans = self.spans_plan.borrow().clone();
        for (kid, _) in spans {
            self.children[kid].el.paint(window, cx);
        }
        // Positioned continuations without a column copy of their ancestors.
        let _ = self.paint_orphans(window);
    }
}
