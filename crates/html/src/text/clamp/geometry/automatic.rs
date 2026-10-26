//! Авто-режим точки среза (`line-clamp: auto` / потолок `max-height`): из потолка высоты
//! получает точку обрыва по css-overflow-4 §5.3 — коробка заданной высоты или с `min-height`
//! за бюджетом прячется целиком, пересечённые коробки без заданной высоты уносят нижние
//! рамку/паддинг/поля, пересечённая строка не показывается половинкой, последняя строка
//! срезается на `text-box-trim`, пустая пересечённая коробка ставит точку перед собой.

use super::geometry::ClampGeometry;
use crate::tolerance::LINE_FIT_EPS;

impl ClampGeometry {
    pub(crate) fn automatic_cut(
        &self,
        mut cut: Option<f32>,
        by_count: bool,
        blocks: &[super::ClampBox],
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
            // So is a box whose `min-height` alone already passes the
            // budget: fragmented anywhere inside, it would still end below
            // it, so the clamp point comes before it (`line-clamp-auto-042`;
            // Blink clamps the ancestor chain size to min/max).
            for b in blocks {
                let monolithic = b.fixed && c2 < b.y1;
                let too_tall = b.min_end > 0.0
                    && b.y0 + b.min_end > c2 + LINE_FIT_EPS
                    && !b.inside_fitting_fixed(blocks, c2);
                if b.y0 < c2 && (monolithic || too_tall) {
                    c2 = b.y0;
                }
            }
            // Коробка БЕЗ заданной высоты фрагментируется по последней
            // влезающей строке, но нижние рамку и паддинг с собой уносит:
            // бюджет строк на них укорачивается, а итоговый срез — на
            // столько же удлиняется (`line-clamp-auto-019`: 2+14+4×32+14+2
            // = 160 = ровно потолок `max-height: 5lh`).
            // A box is crossed while its flow end (end border, padding and
            // margin) does not fit; the end edges of the crossed chain
            // collapse like Blink's `LineClampAncestorChain`
            // (`line-clamp-auto-027/030`).
            let crossed: Vec<&super::ClampBox> = blocks
                .iter()
                .filter(|b| {
                    !b.fixed
                        && b.y0 < c2
                        && c2 < b.flow_end()
                        && !b.inside_fitting_fixed(blocks, c2)
                })
                .collect();
            let bp: f32 = super::chain_extra(&crossed);
            c2 -= bp;
            // Строка, которая влезает только СРЕЗАННОЙ (`text-box-trim:
            // trim-end` — последняя строка перед обрывом срезается), остаётся:
            // `max-height: 285px` при строке 100 и срезе 25 — три строки
            // (3·100 − 25 = 275), а не две (`line-clamp-auto-001/002`).
            for (y0, y1, _) in rows {
                if *y0 < c2 && c2 < *y1 - trim_end - LINE_FIT_EPS {
                    c2 = *y0;
                }
            }
            // И точка обрыва садится на срезанный низ последней видимой
            // строки: при числовом пределе — всегда, в авто-режиме — только
            // когда строки идут дальше точки (иначе обрыва нет и срез уже
            // сделал хвост `blocks()`).
            if trim_end > 0.0
                && (self.limit.is_some() || rows.iter().any(|r| r.1 > c2 + LINE_FIT_EPS))
                && let Some(b) = rows
                    .iter()
                    .map(|r| r.1)
                    .filter(|y1| *y1 - trim_end <= c2 + LINE_FIT_EPS)
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
                    *countable && *r0 >= y0 - LINE_FIT_EPS && *r1 - trim_end <= c2 + LINE_FIT_EPS
                })
            };
            match crossed
                .iter()
                .filter(|b| !holds(b.y0))
                .map(|b| b.y0)
                .reduce(f32::min)
            {
                Some(empty_top) => {
                    let kept: Vec<&super::ClampBox> = crossed
                        .iter()
                        .filter(|b| holds(b.y0) && b.y0 < empty_top)
                        .copied()
                        .collect();
                    cut = Some(empty_top + super::chain_extra(&kept));
                }
                // Crossing is geometric here, and a box sized to the point
                // would stop crossing it on the next frame: the boxes keep
                // their size in this mode (`sizes` stays empty).
                None => cut = Some(super::chain_end(c2, &crossed)),
            }
        }
        cut
    }
}
