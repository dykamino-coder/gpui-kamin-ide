//! `impl Element` для `ColumnStack`: запрос раскладки, `prepaint` (укладка по реальной
//! ширине, расстановка кусков по колонкам, спаннеры во всю ширину, границы для линеек
//! промежутков) и `paint` (маски только у разрезанных детей, позиционированные
//! фрагменты вторым проходом по CSS 2.1 Appendix E). Вертикальная ось уходит в `axis`.

use crate::hooks::Hooks;
use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, Bounds, LayoutId, Pixels, Window, point, px, size};
mod native;
mod paint;
mod prepaint;

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
        let w = f32::from(bounds.size.width);
        let col_w = ((w - self.gap * (self.count as f32 - 1.0)) / self.count as f32).max(1.0);
        if let Some(recipe) = self.deferred.clone() {
            recipe.prepare(self, col_w, window, cx);
        }
        self.column_breaks(col_w, window, cx);
        let heights = self.kid_geoms();
        let (_, lines, mut plan, spans) = self.balance(&heights);
        plan.check_copies(|kid| self.children[kid].frags.len() + 1);
        self.col_w.set(col_w);
        *self.lines_plan.borrow_mut() = lines;
        self.plan_oof(&plan, bounds);
        let step = col_w + self.gap;
        self.assign_flex_owners(&plan);
        self.prepaint_frags(window, cx, bounds, col_w, &plan, step);
        if self
            .prepaint_native(bounds, &plan, &spans, window, cx)
            .is_err()
        {
            plan.failure = Some(crate::layout::fragment::token::Error::InvalidGeometry);
            plan.incomplete();
        }
        // Спаннер — во всю ширину коробки, первой копией (запасных у него
        // нет: между колонками он не режется).
        for &(kid, sy) in &spans {
            if self.native_kid(kid) {
                continue;
            }
            let kid = &mut self.children[kid];
            let h = kid.h;
            kid.el.layout_as_root_at(
                point(bounds.origin.x, bounds.origin.y + px(sy)),
                size(
                    gpui::AvailableSpace::Definite(bounds.size.width),
                    gpui::AvailableSpace::Definite(px(h)),
                ),
                window,
                cx,
            );
            kid.el.prepaint_at(point(px(0.0), px(0.0)), window, cx);
        }
        // Границы для линеек промежутков (css-gaps-1 §gap-multicol): коробки
        // ЗАНЯТЫХ колонок каждой линии (как Blink `AddCrossGap` на колонку
        // ряда; в третьем ряду `column-height-009` линейка одна) и спаннеры
        // во всю ширину — они обрывают линейки колонок.
        self.collect_gap_geometry(bounds, col_w, &plan, &spans);
        *self.plan.borrow_mut() = plan;
        *self.spans_plan.borrow_mut() = spans;
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
        if self.oof.plan.borrow().failure.is_some() || self.paint_native(window, cx).is_err() {
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
            if self.native_kid(kid) {
                continue;
            }
            self.children[kid].el.paint(window, cx);
        }
    }
}
