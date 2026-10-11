//! Automatic for painting; split out to keep the owning module within 250 lines.

use super::ClampCut;

impl ClampCut {
    pub(crate) fn automatic_cut(
        &self,
        mut cut: Option<f32>,
        by_count: bool,
        blocks: &[(f32, f32, bool, f32)],
        rows: &[(f32, f32, bool)],
        trim_end: f32,
    ) -> Option<f32> {
        // Блок, СОДЕРЖАЩИЙ точку среза, фрагментируется по последней
        // влезающей строке — ПРЯЧЕТСЯ целиком только коробка с заданной
        // высотой: её не фрагментировать (css-overflow-4 §line-clamp).
        // Строка, пересечённая точкой, не показывается половинкой:
        // срез поднимается к её верху.
        if let Some(c) = cut.filter(|_| !by_count) {
            // Коробка с ЗАДАННОЙ высотой не фрагментируется: пересечённая
            // точкой среза, она прячется целиком (css-overflow-4 §5.3).
            let mut c2 = c;
            for (y0, y1, fixed, _) in blocks {
                if *fixed && *y0 < c2 && c2 < *y1 {
                    c2 = *y0;
                }
            }
            // Коробка БЕЗ заданной высоты фрагментируется по последней
            // влезающей строке, но нижние рамку и паддинг с собой уносит:
            // бюджет строк на них укорачивается, а итоговый срез — на
            // столько же удлиняется (`line-clamp-auto-019`: 2+14+4×32+14+2
            // = 160 = ровно потолок `max-height: 5lh`).
            let crossed: Vec<(f32, f32)> = blocks
                .iter()
                .filter(|(y0, y1, fixed, _)| !*fixed && *y0 < c2 && c2 < *y1)
                .map(|(y0, _, _, bp)| (*y0, *bp))
                .collect();
            let bp: f32 = crossed.iter().map(|(_, bp)| *bp).sum();
            c2 -= bp;
            // Строка, которая влезает только СРЕЗАННОЙ (`text-box-trim:
            // trim-end` — последняя строка перед обрывом срезается), остаётся:
            // `max-height: 285px` при строке 100 и срезе 25 — три строки
            // (3·100 − 25 = 275), а не две (`line-clamp-auto-001/002`).
            for (y0, y1, _) in rows {
                if *y0 < c2 && c2 < *y1 - trim_end - 0.5 {
                    c2 = *y0;
                }
            }
            // И точка обрыва садится на срезанный низ последней видимой
            // строки: при числовом пределе — всегда, в авто-режиме — только
            // когда строки идут дальше точки (иначе обрыва нет и срез уже
            // сделал хвост `blocks()`).
            if trim_end > 0.0
                && (self.limit.is_some() || rows.iter().any(|r| r.1 > c2 + 0.5))
                && let Some(b) = rows
                    .iter()
                    .map(|r| r.1)
                    .filter(|y1| *y1 - trim_end <= c2 + 0.5)
                    .max_by(|a, b| a.total_cmp(b))
            {
                c2 = c2.min(b - trim_end);
            }
            // Пересечённая коробка, в которую с её верхними рамкой и паддингом
            // не влезло НИ ОДНОЙ строки, не фрагментируется: точка среза — между
            // ней и предыдущим соседом (§5.3 «between two in-flow block-level
            // sibling boxes»; `line-clamp-auto-024`: 224, а не 240). Её нижние
            // рамка и паддинг не нужны; охватывающие непустые свои уносят.
            // Строка «влезла» — по СРЕЗАННОМУ низу (`trim_end`, как в цикле
            // выше): без поправки последняя видимая строка под `text-box-trim`
            // не считалась бы, и срез уходил бы к верху её коробки.
            let holds = |y0: f32| {
                rows.iter().any(|(r0, r1, countable)| {
                    *countable && *r0 >= y0 - 0.5 && *r1 - trim_end <= c2 + 0.5
                })
            };
            match crossed
                .iter()
                .filter(|(y0, _)| !holds(*y0))
                .map(|(y0, _)| *y0)
                .reduce(f32::min)
            {
                Some(empty_top) => {
                    let keep: f32 = crossed
                        .iter()
                        .filter(|(y0, _)| holds(*y0) && *y0 < empty_top)
                        .map(|(_, bp)| *bp)
                        .sum();
                    cut = Some(empty_top + keep);
                }
                None => cut = Some(c2 + bp),
            }
        }
        cut
    }
}
