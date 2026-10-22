//! Раскладка главной копии ребёнка стопки до укладки и распорки роста по ней: высоту
//! ребёнка, чью высоту меряет раскладка (`StackChild::measure`), или точки разрыва его
//! блочной стопки (`StackChild::breaks_w`) даёт разложенная копия; распорки роста ставятся
//! по плану ЭТОЙ стопки (`grow_points`) перед коробкой, найденной в разложенной копии
//! (`push::Regrow`), копии собираются заново и раскладываются, пока план не перестанет
//! требовать роста (css-break-3 §box-splitting; Blink `FinishFragmentation`).

use crate::layout::fragment::types::{BreaksAt, StackChild};
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;
use gpui::{App, AvailableSpace, Window, point, px, size};

/// Заходов роста до неподвижной точки (как у оценки `grow_pushed`).
const PASSES: usize = 6;

mod copies;

impl ColumnStack {
    /// Раскладка главной копии ребёнка `i` в колонку до укладки.
    pub(crate) fn lay_out_main(&mut self, i: usize, window: &mut Window, cx: &mut App) {
        let (axis, fixed) = (self.axis, self.fixed_height);
        let c = &mut self.children[i];
        let measured_size;
        if let Some(w) = c.measure {
            let (sz, content) = c.el.layout_as_root_with_content(
                if axis.is_vertical() {
                    size(AvailableSpace::MaxContent, AvailableSpace::Definite(px(w)))
                } else {
                    size(AvailableSpace::Definite(px(w)), AvailableSpace::MaxContent)
                },
                window,
                cx,
            );
            measured_size = sz;
            let h = f32::from(if axis.is_vertical() {
                sz.width
            } else {
                sz.height
            });
            c.h = h;
            // Видимое переполнение потомков — параллельный поток
            // (css-break-3 §3): продолжается в следующих колонках само.
            c.over = h.max(f32::from(if axis.is_vertical() {
                content.width
            } else {
                content.height
            }));
            if !axis.is_vertical()
                && let Some(w) = &c.watched
            {
                // Floats переполняют нулевую коробку; их реальная протяжённость
                // резервирует продолжение, не растягивая фон normal flow.
                let flow = w.flow.at(point(px(0.0), px(0.0)), sz, axis, window);
                c.over = flow
                    .floats
                    .iter()
                    .map(|g| g.bottom + g.margins[1])
                    .fold(c.over, f32::max);
            }
            if c.regrow.is_some() {
                c.cuts.clear();
                c.forced.clear();
                c.solid.clear();
                c.keep.clear();
            }
            let sections = c.measure_breaks(sz, axis, window);
            c.refresh_repeat(sections, fixed);
            // Целый вложенный многоколоночник выше колонки — не монолит:
            // его режет край внешней колонки (`StackChild::fit_whole`).
            if c.fit_whole {
                c.monolith = fixed.is_none_or(|f| h <= f + 0.01);
            }
        } else if let Some(BreaksAt::Width(w)) = c.breaks_w {
            // Точки разрыва блочной стопки — по раскладке главной копии
            // в колонку (`StackChild::breaks_w`).
            let (sz, content) = c.el.layout_as_root_with_content(
                if axis.is_vertical() {
                    size(AvailableSpace::MaxContent, AvailableSpace::Definite(px(w)))
                } else {
                    size(AvailableSpace::Definite(px(w)), AvailableSpace::MaxContent)
                },
                window,
                cx,
            );
            measured_size = sz;
            #[cfg(feature = "wpt-harness")]
            if c.probe_pending && c.watched.is_some() {
                c.probe.height(
                    "columns.width",
                    c.h,
                    f32::from(if axis.is_vertical() {
                        sz.width
                    } else {
                        sz.height
                    }),
                );
            }
            // Использованный блочный размер — разложенный border box: в нём
            // уже `min-height`/`max-height` и распорки роста. Параллельный
            // поток (css-break-3 §3) остаётся протяжённостью меры.
            let table = c
                .watched
                .as_ref()
                .is_some_and(|w| w.blocks().is_some_and(|b| b.table.is_some()));
            if table {
                // Таблица: высота — разложенные ряды, подписи и заданная
                // высота (css-tables-3 §computing-the-table-height), поток —
                // и видимое переполнение её содержимого (css-break-3 §3).
                let h = f32::from(if axis.is_vertical() {
                    sz.width
                } else {
                    sz.height
                });
                c.h = h;
                c.over = h.max(f32::from(if axis.is_vertical() {
                    content.width
                } else {
                    content.height
                }));
            } else if c.regrow.as_ref().is_some_and(|r| r.laid_h) {
                let h = f32::from(if axis.is_vertical() {
                    sz.width
                } else {
                    sz.height
                });
                c.over = if c.over > c.h + COORD_EPS {
                    c.over.max(h)
                } else {
                    h
                };
                c.h = h;
            }
            let sections = c.replace_breaks(sz, axis, window);
            c.refresh_repeat(sections, fixed);
        } else {
            return;
        }
        if let (Some(r), Some(w)) = (c.regrow.as_mut(), c.watched.as_ref()) {
            r.laid = w.laid_axis(point(px(0.0), px(0.0)), measured_size, axis, window);
            if !r.laid.flexes.is_empty()
                && let Some(blocks) = w.blocks()
            {
                let (before, after) = (blocks.edge(false, &r.laid), blocks.edge(true, &r.laid));
                c.force_before = before.0;
                c.force_after = after.0;
                c.avoid_before = before.1;
                c.avoid_after = after.1;
            }
        }
    }

    /// Распорки роста детей с `regrow` по плану этой стопки.
    pub(crate) fn regrow_children(&mut self, window: &mut Window, cx: &mut App) {
        self.reserve_row_copies();
        if !self.children.iter().any(|c| c.regrow.is_some()) {
            return;
        }
        for _ in 0..PASSES {
            let kids = self.kid_geoms();
            let mut grows = self.grow_points(&kids);
            // Внутри ребёнка — снизу вверх: правка ниже точки не сдвигает
            // коробки выше, и разложенные border box прохода остаются верны.
            grows.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
            let mut dirty = vec![false; self.children.len()];
            let mut local_grows = vec![Vec::new(); self.children.len()];
            let (_, lines, plan, _) = self.balance(&kids);
            for f in &plan {
                let c = &mut self.children[f.kid];
                let (Some(r), Some(w)) = (c.regrow.as_mut(), c.watched.as_ref()) else {
                    continue;
                };
                let Some(blocks) = w.blocks() else { continue };
                let Some(&(_, height)) = lines.get(f.col / self.count) else {
                    continue;
                };
                let edge = f.from + height - f.y;
                let local = crate::layout::fragment::push::flex_flow::growths(
                    blocks,
                    &r.laid,
                    f.from,
                    f.from + f.h,
                    edge,
                );
                local_grows[f.kid].extend(local);
            }
            for (kid, at, grow, forced) in grows {
                let c = &mut self.children[kid];
                let (Some(_r), Some(w)) = (c.regrow.as_mut(), c.watched.as_ref()) else {
                    continue;
                };
                if let Some(blocks) = w.blocks() {
                    local_grows[kid].push((blocks.clone(), at, grow, forced));
                }
            }
            for (index, grows) in local_grows.into_iter().enumerate() {
                if let Some(r) = self.children[index].regrow.as_mut() {
                    dirty[index] = r.push_many(grows);
                }
            }
            if !dirty.contains(&true) {
                break;
            }
            for (i, _) in dirty.iter().enumerate().filter(|d| *d.1) {
                rebuild(&mut self.children[i]);
                self.lay_out_main(i, window, cx);
            }
            self.reserve_row_copies();
        }
    }
}

/// Копии ребёнка из правленого дерева; рекордер забывает прежнюю раскладку.
fn rebuild(c: &mut StackChild) {
    let Some(r) = &c.regrow else {
        return;
    };
    let (el, frags) = r.rebuild(c.frags.len());
    c.el = el;
    c.frags = frags;
    if let Some(w) = &c.watched {
        w.reset();
    }
}
