//! Стопка колонок по оси: предраскладка и краска при произвольной оси стопки.

use super::ColumnStack;
use crate::layout::fragment::fragment_mask;
use crate::layout::fragment::types::{StackAxis, axis_box};
use crate::tolerance::COORD_EPS;
use gpui::{App, Bounds, IntoElement, Pixels, Window, point, px, size};

impl ColumnStack {
    /// Раскладка стопки в ВЕРТИКАЛЬНОМ письме: та же укладка (`balance`),
    /// физика — через `axis_box`. Горизонтальная стопка идёт прежним
    /// `prepaint` байт в байт. Повтор шапок таблицы, строки flex (`Par`),
    /// `clone` и хвост `slack` сюда не приходят: `render::generic_box::multicol` их в вертикали
    /// не взводит.
    #[inline(never)]
    pub(crate) async fn prepaint_axis(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        let axis = self.axis;
        let (inline_avail, block_avail) = self.axis_sizes(bounds);
        let col_w =
            ((inline_avail - self.gap * (self.count as f32 - 1.0)) / self.count as f32).max(1.0);
        if let Some(recipe) = self.deferred.clone() {
            recipe.prepare_async(self, col_w, window, cx, stack).await;
        }
        self.col_w.set(col_w);
        let heights = self.kid_geoms();
        let (_, lines, mut plan, spans) = self.balance(&heights);
        plan.check_copies(|kid| self.children[kid].frags.len() + 1);
        *self.lines_plan.borrow_mut() = lines;
        self.plan_oof(&plan, bounds);
        if self
            .prepaint_native(bounds, &plan, window, cx, stack)
            .await
            .is_err()
        {
            plan.failure = Some(crate::layout::fragment::token::Error::InvalidGeometry);
            plan.incomplete();
        }
        let step = col_w + self.gap;
        let rl = axis == StackAxis::VerticalRl;
        for f in &plan {
            let full_h = self.children[f.kid].h;
            let (c, ry) = self.place(f.col);
            let rel = self.children[f.kid].rel;
            // Копия раскладывается ЦЕЛИКОМ по блочной оси и сдвигается на
            // срез: видимой её часть делает маска (css-break-3 §4, `slice`).
            let b = axis_box(
                axis,
                bounds.origin,
                block_avail,
                c as f32 * step,
                col_w,
                ry + f.y - f.from,
                full_h,
            );
            let kid = &mut self.children[f.kid];
            let measured = kid.measure.is_some()
                || matches!(
                    kid.breaks_w,
                    Some(crate::layout::fragment::types::BreaksAt::Width(_))
                );
            let Some(el) =
                crate::layout::fragment::types::frag_copy(&mut kid.el, &mut kid.frags, f.copy)
            else {
                continue;
            };
            // Строчный размер блока `auto` — во всю колонку (CSS 2.1 §10.3.3
            // в логических осях; css-writing-modes-4 §7.3): у вертикальной
            // копии это ВЫСОТА. Корень `layout_as_root` с гибким рядом
            // (так блок вертикального письма строится в `element()`) высоту
            // по доступному месту не тянет — тянет поперечная ось обёртки
            // (`align-items: stretch`). У `vertical-rl` начало блочной оси —
            // ПРАВЫЙ край обёртки (`flex_row_reverse`).
            // Измеренная копия уже имеет эту обёртку при построении: нельзя
            // переподчинять разложенный корень новому элементу GPUI.
            if !measured {
                use gpui::{ParentElement as _, Styled as _};
                let inner = std::mem::replace(el, gpui::Empty.into_any_element());
                let w = gpui::div().flex().w(b.size.width).h(b.size.height);
                let w = if rl {
                    w.flex_row_reverse()
                } else {
                    w.flex_row()
                };
                *el = w.child(inner).into_any_element();
            }
            // Копия раскладывается ОТ своего абсолютного начала, как у
            // горизонтальной стопки (`layout_as_root_at`): края её коробок
            // округляются в той же сетке устройства, что и маска фрагмента
            // (`fragment_mask::snap`). Прежде округление шло от нуля, а сдвиг
            // на дробное начало колонки добавлялся потом — на стыке колонок
            // оставалась строка точек фона (`transform-001`, `overflow-clip-002`).
            el.layout_as_root_at_async(
                point(b.origin.x + px(rel.0), b.origin.y + px(rel.1)),
                size(
                    gpui::AvailableSpace::Definite(b.size.width),
                    gpui::AvailableSpace::Definite(b.size.height),
                ),
                window,
                cx,
                stack,
            )
            .await;
            el.prepaint_at_async(point(px(0.0), px(0.0)), window, cx, stack)
                .await;
        }
        // Спаннер — во всю СТРОЧНУЮ сторону коробки.
        for &(kid, sy) in &spans {
            let h = self.children[kid].h;
            let b = axis_box(axis, bounds.origin, block_avail, 0.0, inline_avail, sy, h);
            let kid = &mut self.children[kid];
            kid.el
                .layout_as_root_at_async(
                    b.origin,
                    size(
                        gpui::AvailableSpace::Definite(b.size.width),
                        gpui::AvailableSpace::Definite(b.size.height),
                    ),
                    window,
                    cx,
                    stack,
                )
                .await;
            kid.el
                .prepaint_at_async(point(px(0.0), px(0.0)), window, cx, stack)
                .await;
        }
        self.collect_gap_geometry(bounds, col_w, &plan, &spans);
        *self.plan.borrow_mut() = plan;
        *self.spans_plan.borrow_mut() = spans;
    }
}
mod paint;
