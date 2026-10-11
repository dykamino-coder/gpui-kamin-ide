//! `impl Element` для `ColumnStack`.
// owner: A

use crate::layout::fragment::types::{Frag, Kid, RepeatGeom};
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, LayoutId, Pixels, Window,
    point, px, size,
};
mod paint;
mod prepaint;

impl Element for ColumnStack {
    type RequestLayoutState = LayoutId;
    type PrepaintState = Bounds<Pixels>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        self.request_column_layout(window, cx)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) -> Bounds<Pixels> {
        if self.axis.is_vertical() {
            self.prepaint_axis(bounds, window, cx);
            return bounds;
        }
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*state),
            size: window.layout_size_unrounded(*state),
        };
        let w = f32::from(bounds.size.width);
        let col_w = ((w - self.gap * (self.count as f32 - 1.0)) / self.count as f32).max(1.0);
        let heights: Vec<Kid> = self
            .children
            .iter()
            .map(|c| Kid {
                h: c.h,
                mt: c.mt,
                mb: c.mb,
                monolith: c.monolith,
                cuts: c.cuts.clone(),
                force_before: c.force_before,
                force_after: c.force_after,
                avoid_before: c.avoid_before,
                avoid_after: c.avoid_after,
                forced: c.forced.clone(),
                solid: c.solid.clone(),
                span: c.span,
                over: c.over,
                clone_dec: c.clone_dec,
                overflow_top: c.overflow_top,
                repeat: c.repeat.as_ref().map_or(RepeatGeom::default(), |r| r.geom),
                par: c.par,
            })
            .collect();
        let (_, lines, plan, spans) = self.balance(&heights);
        self.col_w.set(col_w);
        *self.lines_plan.borrow_mut() = lines;
        let step = col_w + self.gap;
        self.prepaint_frags(window, cx, bounds, col_w, &plan, step);
        // Спаннер — во всю ширину коробки, первой копией (запасных у него
        // нет: между колонками он не режется).
        for &(kid, sy) in &spans {
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
        self.prepaint_gap_items(bounds, col_w, &plan, &spans, step);
        *self.plan.borrow_mut() = plan;
        *self.spans_plan.borrow_mut() = spans;
        bounds
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        prepaint: &mut Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.axis.is_vertical() {
            self.paint_axis(bounds, window, cx);
            return;
        }
        let bounds = *prepaint;
        // Линейки — по центрам промежутков, высотой в колонку, в каждой линии.
        // Это простая `column-rule` без рядов; при рядах `render.rs` отдаёт
        // линейки (в том числе `row-rule`, css-multicol-2 §rg) художнику
        // `GapRulePainter` по буферу `gap_items`, и `rule` здесь `None`.
        self.paint_column_rules(window, bounds);
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
        let plan_all = plan.clone();
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
    }
}
