//! Возможности разрыва таблицы по её разложенным подписям, коробке и дорожкам рядов.
//!
//! Обёртка — стопка подписей и коробки таблицы: между соседями — точка класса A
//! (css-break-3 §4.3; Blink `table_layout_algorithm.cc` кладёт подписи и коробку одним
//! блочным потоком), поля на разрыве усекаются, `break-*` подписи — разрыв или запрет.
//! Коробка таблицы: верхняя рамка с отбивкой — сплошная (Blink «avoid breaking inside
//! block-start border»); между рядами — точка класса A `(низ ряда, верх следующего)`: зазор
//! `border-spacing` на разрыве усекается (css-tables-3 §fragmentation); граница, закрытая
//! `avoid*` с любой стороны и не открытая принудительным разрывом (css-break-4 §4.3 правило 1,
//! §forced-breaks), — сцепка рядов, которая идёт одним сплошным диапазоном от границы перед
//! первым из них; `break-inside: avoid*` ряда и группы — сплошные диапазоны (правило 2);
//! внутри ряда — точки содержимого ячеек (параллельные потоки, css-break-3 §2.1); нижняя рамка
//! с отбивкой держит примыкающее к низу последнего ряда содержимое. Положения — дорожки сетки,
//! а не оценка: растянутые заданной высотой ряды стоят там, где их положила раскладка.

use super::TableBox;
use crate::layout::fragment::flow::blocks::geometry::{Pending, walk};
use crate::layout::fragment::flow::blocks::{BlockBreaks, BlockKid, Laid, TableFrame};
use crate::tolerance::COORD_EPS;

/// Ребёнок обёртки таблицы: подпись или коробка таблицы.
enum Slot<'a> {
    Caption(&'a BlockKid),
    Table,
}

impl TableBox {
    /// Точки обёртки, стоящей с `top`.
    pub(in crate::layout::fragment::flow::blocks) fn walk(
        &self,
        top: f32,
        laid: &Laid,
        out: &mut BlockBreaks,
    ) {
        let frame = laid.table(self.key);
        let slots = self
            .caps_top
            .iter()
            .map(Slot::Caption)
            .chain(std::iter::once(Slot::Table))
            .chain(self.caps_bot.iter().map(Slot::Caption));
        // Предыдущий сосед: `break-after` принудительный и `avoid*`, его
        // border box.
        let mut prev: Option<(bool, bool, f32, f32)> = None;
        for slot in slots {
            let place = match &slot {
                Slot::Caption(k) => laid.at(k.id).map(|(t, b)| {
                    let edges = (k.force_before, k.force_after, k.avoid_before, k.avoid_after);
                    (t, b, edges)
                }),
                Slot::Table => frame.map(|f| (f.table.0, f.table.1, (false, false, false, false))),
            };
            let Some((st, sb, (fb, fa, ab, aa))) = place else {
                prev = None;
                continue;
            };
            if let Some((pfa, paa, pt, pb)) = prev {
                out.cuts.push((pb, st));
                if pfa || fb {
                    out.forced.push(pb);
                } else if paa || ab {
                    out.pending.push(Pending::Avoid {
                        top: pt,
                        bottom: pb,
                        floor: top,
                        next: st,
                    });
                }
            }
            match slot {
                Slot::Caption(k) if k.monolith => out.solid.push((st, sb)),
                Slot::Caption(k) => {
                    if let Some(inner) = &k.inner {
                        walk(inner, st, sb, laid, out);
                    }
                }
                Slot::Table => {
                    if let Some(f) = frame {
                        self.walk_rows(f, laid, out);
                    }
                }
            }
            prev = Some((fa, aa, st, sb));
        }
    }

    /// Точки коробки таблицы по её разложенному кадру `f`.
    fn walk_rows(&self, f: &TableFrame, laid: &Laid, out: &mut BlockBreaks) {
        let (table_top, table_bottom) = f.table;
        let (grid_top, grid_bottom) = f.grid;
        if grid_top > table_top + COORD_EPS {
            out.solid.push((table_top, grid_top));
        }
        // Конец предыдущего ряда (у первого — верх содержимого коробки).
        let mut y = grid_top;
        let mut group_open: Option<(usize, f32)> = None;
        let mut avoid_run: Option<f32> = None;
        let mut prev_open = table_top;
        let mut prev_avoid_after = false;
        let mut force_next = false;
        for (i, r) in self.rows.iter().enumerate() {
            let Some(&(start, end)) = f.rows.get(i) else {
                break;
            };
            let force = r.force_before || force_next;
            let joined = i > 0 && (prev_avoid_after || r.avoid_before) && !force;
            if i > 0 {
                if !joined {
                    out.cuts.push((y, start));
                }
                if force {
                    out.forced.push(y);
                }
            }
            force_next = r.force_after;
            // Граница перед рядом; у первого — верх коробки таблицы: край
            // фрагментаинера в сцепке с ним — разрыв перед таблицей.
            let open = if i == 0 { table_top } else { y };
            // Принудительный разрыв сильнее запрета внутри группы
            // (css-break-4 §forced-breaks): её сплошной диапазон делится им.
            if i > 0
                && force
                && let Some((g, gs)) = group_open
                && g == r.group
            {
                out.solid.push((gs, y));
                group_open = Some((g, open));
            }
            if joined {
                avoid_run.get_or_insert(prev_open);
            } else if let Some(s) = avoid_run.take() {
                out.solid.push((s, y));
            }
            if r.avoid_inside {
                out.solid.push((open, end));
            }
            if let Some((g, gs)) = group_open
                && g != r.group
            {
                out.solid.push((gs, y));
                group_open = None;
            }
            if r.group_avoid && group_open.is_none() {
                group_open = Some((r.group, open));
            }
            prev_open = open;
            prev_avoid_after = r.avoid_after;
            for (span, cell) in &r.cells {
                let bottom = f.rows.get(i + span.saturating_sub(1)).map_or(end, |r| r.1);
                walk(cell, start, bottom, laid, out);
            }
            y = end;
        }
        if let Some(s) = avoid_run {
            out.solid.push((s, y));
        }
        if let Some((_, gs)) = group_open {
            out.solid.push((gs, y));
        }
        // Первый ряд без зазора под верхней рамкой с отбивкой: разрыва между
        // ними нет (Blink кладёт первую секцию без разделения с началом
        // коробки таблицы), монолит ряда тянет рамку за собой.
        if grid_top > table_top + COORD_EPS
            && let Some(&(first, _)) = f.rows.first()
            && (first - grid_top).abs() < COORD_EPS
        {
            out.pending.push(Pending::Head {
                top: table_top,
                edge: first,
            });
        }
        // Нижняя рамка с отбивкой таблицы приклеена к содержимому, кончающемуся
        // на низу последнего ряда (класс C — только при ненулевом зазоре,
        // css-break-4 §possible-breaks; Blink `FinishFragmentation`).
        if table_bottom > grid_bottom + COORD_EPS {
            out.pending.push(Pending::Tail {
                top: table_top,
                edge: y,
                from: grid_bottom,
                bottom: table_bottom,
                gapless: true,
            });
        }
    }

    /// Самая внешняя коробка таблицы, чей border box начинается ровно в `a`:
    /// подпись или коробка в стопке подписи либо ячейки ряда, внутри которого
    /// лежит `a` (ряд растёт вместе с ячейкой, css-tables-3
    /// §height-distribution). Ряд, перенесённый целиком разрывом на границе
    /// рядов, не растёт: хвост фрагмента таблицы — её фон (`StackChild::slack`).
    pub(in crate::layout::fragment::flow::blocks) fn pushed_at(
        &self,
        a: f32,
        laid: &Laid,
    ) -> Option<u64> {
        for k in self.caps_top.iter().chain(&self.caps_bot) {
            let Some((top, bottom)) = laid.at(k.id) else {
                continue;
            };
            if (top - a).abs() < COORD_EPS {
                return Some(k.id);
            }
            if top < a && a < bottom - COORD_EPS {
                return k.inner.as_ref()?.pushed_at(a, laid);
            }
        }
        let f = laid.table(self.key)?;
        self.rows
            .iter()
            .zip(&f.rows)
            .enumerate()
            .find_map(|(i, (r, &(start, _)))| {
                r.cells.iter().find_map(|(span, cell)| {
                    // Ячейка с охватом стоит до низа последнего охваченного ряда.
                    let end = f.rows.get(i + span.saturating_sub(1))?.1;
                    (a > start + COORD_EPS && a < end - COORD_EPS)
                        .then(|| cell.pushed_at(a, laid))
                        .flatten()
                })
            })
    }
}
