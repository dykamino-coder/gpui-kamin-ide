//! Чистая геометрия clamp по опросам боксов и абзацев текущего кадра.

use super::{ClampBox, ClampEntry, fit, paragraph};
use std::collections::HashMap;

#[derive(Clone)]
pub(crate) struct ClampGeometry {
    pub limit: Option<u32>,
    pub max_h: Option<f32>,
    pub trim_end: f32,
}

pub(super) use crate::text::clamp::ClampPlan;

impl ClampGeometry {
    pub fn compute(
        &self,
        all_entries: Vec<ClampEntry>,
        para_rows: &HashMap<u32, Vec<(f32, f32)>>,
        top: f32,
    ) -> ClampPlan {
        let empties: Vec<ClampEntry> = all_entries.iter().filter(|e| e.empty).cloned().collect();
        let mut entries: Vec<ClampEntry> = all_entries.into_iter().filter(|e| !e.empty).collect();
        // A `max-height` box whose content overflows it is sized by the
        // constraint alone: monolithic for the clamp point.
        let overflowing: Vec<bool> = entries
            .iter()
            .map(|b| {
                b.capped
                    && b.span.is_some_and(|(a, z)| {
                        let end = f32::from(b.bounds.origin.y) + f32::from(b.bounds.size.height);
                        entries.iter().any(|e| {
                            e.seq.is_some_and(|s| (a..z).contains(&s))
                                && f32::from(e.bounds.origin.y) + f32::from(e.bounds.size.height)
                                    > end + 0.5
                        })
                    })
            })
            .collect();
        for (e, o) in entries.iter_mut().zip(overflowing) {
            e.fixed_height |= o;
        }
        // Строки текстового вклада: настоящие, если абзац их сообщил и они
        // лежат в его коробке; иначе — высота пробы поровну на строки.
        let split = |e: &ClampEntry| -> Vec<(f32, f32)> {
            let y0 = f32::from(e.bounds.origin.y);
            let h = f32::from(e.bounds.size.height);
            if let Some(r) = e.seq.and_then(|s| para_rows.get(&s))
                && !r.is_empty()
                && r.iter().all(|(a, b)| *a >= y0 - 0.5 && *b <= y0 + h + 0.5)
            {
                return r.clone();
            }
            let n = (h / e.line).round().max(1.0) as usize;
            let step = h / n as f32;
            (0..n)
                .map(|i| (y0 + i as f32 * step, y0 + (i + 1) as f32 * step))
                .collect()
        };
        // Строки: у текстового вклада их bounds.height / line штук.
        let mut rows: Vec<(f32, f32, bool)> = vec![]; // (верх, низ, считается)
        let mut blocks: Vec<ClampBox> = vec![];
        let fixed = fit::fixed_blocks(&entries);
        let fit = |e: &ClampEntry| fit::park_overflow(split(e), e, &fixed);
        for e in &entries {
            let y0 = f32::from(e.bounds.origin.y);
            let h = f32::from(e.bounds.size.height);
            if e.line > 0.0 && h > 0.0 {
                for (a, b) in fit(e) {
                    rows.push((a, b, !e.skip_count));
                }
            } else if h > 0.0 {
                blocks.push(ClampBox {
                    y0: y0 - e.bt_before,
                    y1: y0 + h,
                    fixed: e.fixed_height,
                    bp: e.bp_after,
                    margin: e.end_margin,
                    min_end: e.min_end,
                    node: e.node,
                    span: e.span,
                });
            }
        }
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut cut: Option<f32> = self.max_h.map(|m| top + m);
        let trim_end = self.trim_end;
        // Счётный режим (css-overflow-4 §5.3): точка среза — СРАЗУ после N-й
        // считаемой строки. Это готовая строка, а не бюджет высоты, поэтому
        // правила авто-режима ниже (вычет нижних рамки и паддинга, посадка на
        // верх пересечённой строки) к ней не применяются: после `c2 -= bp`
        // точка уходила внутрь N-й строки, и строка пропадала
        // (`line-clamp-012/022`, `webkit-line-clamp-050`). Абзацу N-й строки
        // отдаётся бюджет строк — как и у авто-режима, и
        // «…» ставит строчный слой. Высоту даёт УКОРОЧЕННОЕ содержимое — с
        // полями, схлопыванием и заданной высотой предков (Blink: всё за
        // точкой `is_hidden_for_paint`, размер — по видимому); потолок нужен,
        // только когда за точкой есть другое содержимое.
        let mut sizes: Vec<(u64, f32)> = vec![];
        let (counted_cut, num_para, by_count) =
            self.counted_cut(cut, &entries, &blocks, &split, &mut sizes);
        cut = counted_cut;
        cut = self.automatic_cut(cut, by_count, &blocks, &rows, trim_end);

        let para = paragraph::automatic_para(cut, by_count, &entries, &empties, fit, trim_end);
        let para = if by_count { num_para } else { para };
        // A clamp point between blocks: the first paragraph after it, so
        // that what follows is hidden in box-tree order and what precedes
        // keeps its overflow.
        let next = cut
            .filter(|c| para.is_none() && c.is_finite())
            .and_then(|c| {
                entries
                    .iter()
                    .filter(|e| {
                        !e.skip_count && e.line > 0.0 && f32::from(e.bounds.origin.y) >= c - 0.5
                    })
                    .filter_map(|e| e.seq)
                    .min()
            });
        let extent = match next {
            Some(n) => fit::extent_before_next(n, &entries, top),
            None => fit::extent_before_point(cut.zip(para), &entries, &split, top),
        };

        ClampPlan {
            nodes: blocks.iter().map(|b| b.node).filter(|n| *n != 0).collect(),
            sizes,
            para,
            next,
            extent,
            cut: cut.map(|c| (c - top).max(0.0)).unwrap_or(f32::INFINITY),
        }
    }
}
