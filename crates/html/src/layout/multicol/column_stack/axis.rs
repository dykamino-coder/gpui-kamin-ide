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
    pub(crate) fn prepaint_axis(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let axis = self.axis;
        let (inline_avail, block_avail) = self.axis_sizes(bounds);
        let col_w =
            ((inline_avail - self.gap * (self.count as f32 - 1.0)) / self.count as f32).max(1.0);
        if let Some(recipe) = self.deferred.clone() {
            recipe.prepare(self, col_w, window, cx);
        }
        let heights = self.kid_geoms();
        let (_, lines, mut plan, spans) = self.balance(&heights);
        plan.check_copies(|kid| self.children[kid].frags.len() + 1);
        self.col_w.set(col_w);
        *self.lines_plan.borrow_mut() = lines;
        self.plan_oof(&plan, bounds);
        if self.prepaint_native(bounds, &plan, window, cx).is_err() {
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
            el.layout_as_root_at(
                point(b.origin.x + px(rel.0), b.origin.y + px(rel.1)),
                size(
                    gpui::AvailableSpace::Definite(b.size.width),
                    gpui::AvailableSpace::Definite(b.size.height),
                ),
                window,
                cx,
            );
            el.prepaint_at(point(px(0.0), px(0.0)), window, cx);
        }
        // Спаннер — во всю СТРОЧНУЮ сторону коробки.
        for &(kid, sy) in &spans {
            let h = self.children[kid].h;
            let b = axis_box(axis, bounds.origin, block_avail, 0.0, inline_avail, sy, h);
            let kid = &mut self.children[kid];
            kid.el.layout_as_root_at(
                b.origin,
                size(
                    gpui::AvailableSpace::Definite(b.size.width),
                    gpui::AvailableSpace::Definite(b.size.height),
                ),
                window,
                cx,
            );
            kid.el.prepaint_at(point(px(0.0), px(0.0)), window, cx);
        }
        self.collect_gap_geometry(bounds, col_w, &plan, &spans);
        *self.plan.borrow_mut() = plan;
        *self.spans_plan.borrow_mut() = spans;
    }

    /// Отрисовка вертикальной стопки (пара к `prepaint_axis`).
    pub(crate) fn paint_axis(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        if self.oof.plan.borrow().failure.is_some() || self.record_native(window, cx).is_err() {
            return;
        }
        let axis = self.axis;
        let (_, block_avail) = self.axis_sizes(bounds);
        let col_w = self.col_w.get();
        let step = col_w + self.gap;
        let plan = self.plan.borrow().clone();
        let plan_all = plan.clone();
        let mut parts = vec![0usize; self.children.len()];
        for f in &plan {
            parts[f.kid] += 1;
        }
        let nested_frag = crate::layout::fragment::effect_fragment::active(&self.paint);
        let records = self.copy_records();
        for f in plan {
            let (c, ry) = self.place(f.col);
            let rel = self.children[f.kid].rel;
            let split = parts[f.kid] > 1;
            // Срез поперёк БЛОЧНОЙ оси; вдоль строчной колонка переполнение
            // не режет (css-multicol-1 §8.1 «visibly overflows and is not
            // clipped to the column box») — вылет на высоту окна, как у
            // горизонтальной стопки на его ширину. Кроме вложенного
            // многоколоночника (`nested_cols`).
            let win_h = f32::from(window.viewport_size().height);
            let spill = if self.children[f.kid].nested_cols {
                0.0
            } else {
                win_h.max(f32::from(bounds.size.height))
            };
            // Чернильное переполнение над первым и под последним фрагментом
            // видно — как у горизонтальной стопки (`paint_frags`).
            let continued = plan_all
                .iter()
                .any(|g| g.kid == f.kid && g.copy == f.copy + 1);
            let line_h = self.lines_plan.borrow().first().map_or(0.0, |l| l.1);
            let k0 = &self.children[f.kid];
            // Измерение размера не обрезает чернильное переполнение краёв
            // без разрыва (css-break-3 §5.4). Свой clip задаёт вложенная стопка.
            let keep = spill == 0.0 || nested_frag;
            let up = if !keep && f.copy == 0 && f.from <= COORD_EPS {
                spill
            } else {
                0.0
            };
            let down = if keep || continued || f.from + f.h < k0.over - COORD_EPS {
                0.0
            } else {
                (line_h - f.y - f.h).max(0.0)
            };
            let b = axis_box(
                axis,
                bounds.origin,
                block_avail,
                c as f32 * step - spill,
                col_w + spill + spill,
                ry + f.y - up,
                f.h + up + down,
            );
            let mask = fragment_mask::snap(
                Bounds {
                    origin: point(b.origin.x + px(rel.0), b.origin.y + px(rel.1)),
                    size: b.size,
                },
                window.scale_factor(),
            );
            let kid = &mut self.children[f.kid];
            let Some(el) =
                crate::layout::fragment::types::frag_copy(&mut kid.el, &mut kid.frags, f.copy)
            else {
                continue;
            };
            let segment = gpui::PaintSegmentId(f.col as u64);
            let mut paint = |window: &mut Window| {
                crate::paint::continuation::with_copy_segment(
                    segment,
                    records.as_ref(),
                    window,
                    |window| el.paint(window, cx),
                )
            };
            if split {
                window.with_content_mask(Some(mask), paint);
            } else {
                paint(window);
            }
        }
        let spans = self.spans_plan.borrow().clone();
        for (kid, _) in spans {
            self.children[kid].el.paint(window, cx);
        }
        let _ = self.paint_orphans(window);
    }
}
