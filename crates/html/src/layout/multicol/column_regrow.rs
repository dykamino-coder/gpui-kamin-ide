//! Раскладка главной копии ребёнка стопки до укладки и распорки роста по ней: высоту
//! ребёнка, чью высоту меряет раскладка (`StackChild::measure`), или точки разрыва его
//! блочной стопки (`StackChild::breaks_w`) даёт разложенная копия; распорки роста ставятся
//! по плану ЭТОЙ стопки (`grow_points`) перед коробкой, найденной в разложенной копии
//! (`push::Regrow`), копии собираются заново и раскладываются, пока план не перестанет
//! требовать роста (css-break-3 §box-splitting; Blink `FinishFragmentation`).

use crate::layout::fragment::types::StackChild;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;
use gpui::{App, AvailableSpace, Window, point, px, size};

/// Заходов роста до неподвижной точки (как у оценки `grow_pushed`).
const PASSES: usize = 6;

impl ColumnStack {
    /// Раскладка главной копии ребёнка `i` в колонку до укладки.
    pub(crate) fn lay_out_main(&mut self, i: usize, window: &mut Window, cx: &mut App) {
        let (axis, fixed) = (self.axis, self.fixed_height);
        let c = &mut self.children[i];
        if let Some(w) = c.measure {
            let (sz, content) = c.el.layout_as_root_with_content(
                size(AvailableSpace::Definite(px(w)), AvailableSpace::MaxContent),
                window,
                cx,
            );
            let h = f32::from(sz.height);
            c.h = h;
            // Видимое переполнение потомков — параллельный поток
            // (css-break-3 §3): продолжается в следующих колонках само.
            c.over = h.max(f32::from(content.height));
            if c.regrow.is_some() {
                c.cuts.clear();
                c.forced.clear();
                c.solid.clear();
            }
            let sections = c.measure_breaks(sz, axis, window);
            c.refresh_repeat(sections, fixed);
            // Целый вложенный многоколоночник выше колонки — не монолит:
            // его режет край внешней колонки (`StackChild::fit_whole`).
            if c.fit_whole {
                c.monolith = fixed.is_none_or(|f| h <= f + 0.01);
            }
        } else if let Some(w) = c.breaks_w {
            // Точки разрыва блочной стопки — по раскладке главной копии
            // в колонку (`StackChild::breaks_w`).
            let (sz, content) = c.el.layout_as_root_with_content(
                size(AvailableSpace::Definite(px(w)), AvailableSpace::MaxContent),
                window,
                cx,
            );
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
                let h = f32::from(sz.height);
                c.h = h;
                c.over = h.max(f32::from(content.height));
            } else if c.regrow.as_ref().is_some_and(|r| r.laid_h) {
                let h = f32::from(sz.height);
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
            r.laid = w.laid(point(px(0.0), px(0.0)), window);
        }
    }

    /// Распорки роста детей с `regrow` по плану этой стопки.
    pub(crate) fn regrow_children(&mut self, window: &mut Window, cx: &mut App) {
        if self.axis.is_vertical() || !self.children.iter().any(|c| c.regrow.is_some()) {
            return;
        }
        for _ in 0..PASSES {
            let kids = self.kid_geoms();
            let mut grows = self.grow_points(&kids);
            // Внутри ребёнка — снизу вверх: правка ниже точки не сдвигает
            // коробки выше, и разложенные border box прохода остаются верны.
            grows.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
            let mut dirty = vec![false; self.children.len()];
            for (kid, at, grow, forced) in grows {
                let c = &mut self.children[kid];
                let (Some(r), Some(w)) = (c.regrow.as_mut(), c.watched.as_ref()) else {
                    continue;
                };
                if let Some(blocks) = w.blocks()
                    && r.push(blocks, at, grow, forced)
                {
                    dirty[kid] = true;
                }
            }
            if !dirty.contains(&true) {
                break;
            }
            for (i, _) in dirty.iter().enumerate().filter(|d| *d.1) {
                rebuild(&mut self.children[i]);
                self.lay_out_main(i, window, cx);
            }
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
