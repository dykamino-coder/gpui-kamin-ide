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
mod piece;
mod prepaint;
mod prepaint_piece;
mod replay;

impl ColumnStack {
    /// Горизонтальная укладка в колонки. Кадры, живые во время вложенных
    /// раскладок и prepaint копий (`prepare`, `column_breaks`,
    /// `prepaint_frags`), держатся малыми: копия может сама нести
    /// многоколоночник, и его prepaint идёт внутри этого.
    #[inline(never)]
    async fn prepaint_columns(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        let w = f32::from(bounds.size.width);
        let col_w = ((w - self.gap * (self.count as f32 - 1.0)) / self.count as f32).max(1.0);
        self.measure_columns_async(col_w, window, cx, stack).await;
        let planned = self.plan_columns(bounds, col_w);
        let step = col_w + self.gap;
        self.prepaint_frags(window, cx, stack, bounds, (col_w, step), &planned.0)
            .await;
        self.finish_prepaint(bounds, col_w, planned, window, cx, stack)
            .await;
    }

    /// Меры детей по реальной ширине колонки (отложенная стопка и точки разрыва).
    #[inline(never)]
    async fn measure_columns_async(
        &mut self,
        col_w: f32,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        if let Some(recipe) = self.deferred.clone() {
            recipe.prepare_async(self, col_w, window, cx, stack).await;
        }
        self.column_breaks_async(col_w, window, cx, stack).await;
    }

    /// Native OOF, спаннеры и линейки — после prepaint копий.
    #[inline(never)]
    async fn finish_prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        col_w: f32,
        planned: (crate::layout::fragment::plan::Plan, Vec<(usize, f32)>),
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        let (mut plan, spans) = planned;
        if self
            .prepaint_native(bounds, &plan, window, cx, stack)
            .await
            .is_err()
        {
            plan.failure = Some(crate::layout::fragment::token::Error::InvalidGeometry);
            plan.incomplete();
        }
        self.prepaint_spanners(bounds, col_w, (plan, spans), window, cx, stack)
            .await;
    }
}

impl Hooks for ColumnStack {
    type Layout = LayoutId;
    type Prepaint = Bounds<Pixels>;

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, Self::Layout) {
        self.request_column_layout_async(window, cx, stack).await
    }

    async fn prepaint_async(
        &mut self,
        _bounds: Bounds<Pixels>,
        state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Self::Prepaint {
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*state),
            size: window.layout_size_unrounded(*state),
        };
        if self.axis.is_vertical() {
            self.prepaint_axis(bounds, window, cx, stack).await;
            return bounds;
        }
        self.prepaint_columns(bounds, window, cx, stack).await;
        bounds
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        state: &mut Self::Layout,
        prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::hooks::paint_on_stack(self, bounds, state, prepaint, window, cx);
    }

    fn paint_async<'a>(
        &'a mut self,
        _bounds: Bounds<Pixels>,
        _state: &'a mut Self::Layout,
        prepaint: &'a mut Self::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a> {
        Box::pin(async move {
            let bounds = *prepaint;
            if self.axis.is_vertical() {
                self.paint_axis(bounds, window, cx, stack).await;
                return;
            }
            if self.oof.plan.borrow().failure.is_some()
                || self.record_native_async(window, cx, stack).await.is_err()
            {
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
            self.paint_frags(
                window, cx, bounds, col_w, step, parts, plan_all, plan, stack,
            )
            .await;
            let spans = self.spans_plan.borrow().clone();
            for (kid, _) in spans {
                self.children[kid].el.paint_async(window, cx, stack).await;
            }
            // Positioned continuations without a column copy of their ancestors.
            let _ = self.paint_orphans(window);
        })
    }
}
