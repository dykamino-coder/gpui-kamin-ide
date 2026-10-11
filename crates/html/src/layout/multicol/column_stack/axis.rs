//! Стопка колонок по оси: предраскладка и краска при произвольной оси стопки.

use super::ColumnStack;
use crate::layout::fragment::fragment_mask;
use crate::layout::fragment::types::{StackAxis, axis_box};
use gpui::{App, Bounds, IntoElement, Pixels, Window, point, px, size};

impl ColumnStack {
    /// Раскладка стопки в ВЕРТИКАЛЬНОМ письме: та же укладка (`balance`),
    /// физика — через `axis_box`. Горизонтальная стопка идёт прежним
    /// `prepaint` байт в байт. Повтор шапок таблицы, строки flex (`Par`),
    /// `clone` и хвост `slack` сюда не приходят: `render.rs` их в вертикали
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
        let heights = self.kid_geoms();
        let (_, lines, plan, spans) = self.balance(&heights);
        self.col_w.set(col_w);
        *self.lines_plan.borrow_mut() = lines;
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
            let el = if f.copy == 0 {
                &mut kid.el
            } else {
                match kid.frags.get_mut(f.copy - 1) {
                    Some(e) => e,
                    None => continue,
                }
            };
            // Строчный размер блока `auto` — во всю колонку (CSS 2.1 §10.3.3
            // в логических осях; css-writing-modes-4 §7.3): у вертикальной
            // копии это ВЫСОТА. Корень `layout_as_root` с гибким рядом
            // (так блок вертикального письма строится в `element()`) высоту
            // по доступному месту не тянет — тянет поперечная ось обёртки
            // (`align-items: stretch`). У `vertical-rl` начало блочной оси —
            // ПРАВЫЙ край обёртки (`flex_row_reverse`).
            {
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
        if let Some(items) = &self.gap_items {
            let lines = self.lines_plan.borrow();
            let mut used = vec![0usize; lines.len()];
            for f in &plan {
                if let Some(u) = used.get_mut(f.col / self.count) {
                    *u = (*u).max(f.col % self.count + 1);
                }
            }
            let mut items = items.borrow_mut();
            for (l, &(ly, lh)) in lines.iter().enumerate() {
                for c in 0..used[l].max(1) {
                    items.push(axis_box(
                        axis,
                        bounds.origin,
                        block_avail,
                        c as f32 * step,
                        col_w,
                        ly,
                        lh,
                    ));
                }
            }
            for &(kid, sy) in &spans {
                items.push(axis_box(
                    axis,
                    bounds.origin,
                    block_avail,
                    0.0,
                    inline_avail,
                    sy,
                    self.children[kid].h,
                ));
            }
        }
        *self.plan.borrow_mut() = plan;
        *self.spans_plan.borrow_mut() = spans;
    }

    /// Отрисовка вертикальной стопки (пара к `prepaint_axis`).
    pub(crate) fn paint_axis(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let axis = self.axis;
        let (_, block_avail) = self.axis_sizes(bounds);
        let col_w = self.col_w.get();
        let step = col_w + self.gap;
        let wrap = matches!(self.rows, Some(r) if r.wrap);
        if let Some((rw, color)) = self.rule {
            let rows = self.lines_plan.borrow().clone();
            // css-multicol-1 §4: линейка только между ЗАНЯТЫМИ колонками.
            let used: Vec<usize> = {
                let plan = self.plan.borrow();
                (0..rows.len())
                    .map(|l| {
                        plan.iter()
                            .filter(|f| {
                                f.h > 0.01 && (if wrap { f.col / self.count } else { 0 }) == l
                            })
                            .map(|f| if wrap { f.col % self.count } else { f.col } + 1)
                            .max()
                            .unwrap_or(0)
                    })
                    .collect()
            };
            for (l, &(ry, rh)) in rows.iter().enumerate() {
                for i in 1..used.get(l).copied().unwrap_or(0).min(self.count) {
                    // Толщина линейки идёт по СТРОЧНОЙ оси (она стоит в
                    // промежутке между колонками), длина — по блочной.
                    let io = i as f32 * step - self.gap * 0.5 - rw * 0.5;
                    window.paint_quad(gpui::fill(
                        axis_box(axis, bounds.origin, block_avail, io, rw, ry, rh),
                        color,
                    ));
                }
            }
        }
        let plan = self.plan.borrow().clone();
        let mut parts = vec![0usize; self.children.len()];
        for f in &plan {
            parts[f.kid] += 1;
        }
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
            let b = axis_box(
                axis,
                bounds.origin,
                block_avail,
                c as f32 * step - spill,
                col_w + spill + spill,
                ry + f.y,
                f.h,
            );
            let mask = fragment_mask::snap(
                Bounds {
                    origin: point(b.origin.x + px(rel.0), b.origin.y + px(rel.1)),
                    size: b.size,
                },
                window.scale_factor(),
            );
            let kid = &mut self.children[f.kid];
            let el = if f.copy == 0 {
                &mut kid.el
            } else {
                match kid.frags.get_mut(f.copy - 1) {
                    Some(e) => e,
                    None => continue,
                }
            };
            if split {
                window.with_content_mask(Some(mask), |window| el.paint(window, cx));
            } else {
                el.paint(window, cx);
            }
        }
        let spans = self.spans_plan.borrow().clone();
        for (kid, _) in spans {
            self.children[kid].el.paint(window, cx);
        }
    }
}
