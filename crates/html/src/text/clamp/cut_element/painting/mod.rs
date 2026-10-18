//! Clamp paint feedback: measured cuts, paragraph budgets and next-frame geometry.

mod automatic;
mod boxes;
mod counted;
mod fit;
mod paragraph;
pub(crate) use boxes::ClampBox;
pub(super) use boxes::{chain_end, chain_extra, chain_sizes};

use super::ClampCut;
use super::{ClampEntry, clamp_cut, clamp_para, take_para_rows};
use crate::text::clamp::line_state::{CLAMP_BOXES, CLAMP_EXTENT, CLAMP_NEXT};
use crate::text::clamp::{CLAMP_CUTS, CLAMP_PARA};
use gpui::{App, Bounds, Pixels, Window};

impl ClampCut {
    // Keep the extracted GPUI Element paint signature intact.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_impl(
        &mut self,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        let all_entries = std::mem::take(&mut *self.lines.borrow_mut());
        // Пустые коробки — только для выбора знака обрыва (ниже); в строки,
        // блоки и признак «за точкой есть содержимое» они не входят.
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
        let para_rows = take_para_rows(self.key);
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
        let top = f32::from(bounds.origin.y);
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
        rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut cut: Option<f32> = self.max_h.map(|m| top + m);
        let trim_end = self.trim_end;
        // Счётный режим (css-overflow-4 §5.3): точка среза — СРАЗУ после N-й
        // считаемой строки. Это готовая строка, а не бюджет высоты, поэтому
        // правила авто-режима ниже (вычет нижних рамки и паддинга, посадка на
        // верх пересечённой строки) к ней не применяются: после `c2 -= bp`
        // точка уходила внутрь N-й строки, и строка пропадала
        // (`line-clamp-012/022`, `webkit-line-clamp-050`). Абзацу N-й строки
        // отдаётся бюджет строк — тот же `CLAMP_PARA`, что у авто-режима, и
        // «…» ставит строчный слой. Высоту даёт УКОРОЧЕННОЕ содержимое — с
        // полями, схлопыванием и заданной высотой предков (Blink: всё за
        // точкой `is_hidden_for_paint`, размер — по видимому); потолок нужен,
        // только когда за точкой есть другое содержимое.
        let mut sizes: Vec<(u64, f32)> = vec![];
        let (counted_cut, num_para, by_count) =
            self.counted_cut(cut, &entries, &blocks, &split, &mut sizes);
        cut = counted_cut;
        cut = self.automatic_cut(cut, by_count, &blocks, &rows, trim_end);
        // The boxes fragmented at the point end there too.
        let mut boxes_moved = false;
        CLAMP_BOXES.with(|m| {
            let mut m = m.borrow_mut();
            let mine: Vec<u64> = blocks.iter().map(|b| b.node).filter(|n| *n != 0).collect();
            for n in &mine {
                let now = sizes.iter().find(|(k, _)| k == n).map(|(_, h)| *h);
                let before = m.get(n).copied();
                let differs = match (before, now) {
                    (Some(a), Some(b)) => (a - b).abs() > 0.5,
                    (a, b) => a.is_some() != b.is_some(),
                };
                if differs {
                    boxes_moved = true;
                    match now {
                        Some(h) => m.insert(*n, h),
                        None => m.remove(n),
                    };
                }
            }
        });
        if boxes_moved {
            window.request_animation_frame();
        }
        let para = paragraph::automatic_para(cut, by_count, &entries, &empties, fit, trim_end);
        // Бюджет одного и того же абзаца только УЖИМАЕТСЯ: рост числа
        // строк на следующем кадре — это отражение нашей же правки, а не
        // новое измерение. Правило конечно (бюджет строго убывает и не
        // меньше единицы), поэтому кадр не может просить себя без конца.
        // Счётный режим несёт свой бюджет (см. выше); авто-режим — свой.
        let para = if by_count { num_para } else { para };
        let prev_para = clamp_para(self.key);
        let para = match (prev_para, para) {
            (Some((ps, pk)), Some((s, k))) if ps == s && k > pk => Some((ps, pk)),
            (_, v) => v,
        };
        if prev_para != para {
            CLAMP_PARA.with(|m| {
                let mut m = m.borrow_mut();
                match para {
                    Some(v) => {
                        m.insert(self.key, v);
                    }
                    None => {
                        m.remove(&self.key);
                    }
                }
            });
            window.request_animation_frame();
        }
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
        if crate::text::clamp::line_state::clamp_next(self.key) != next {
            CLAMP_NEXT.with(|m| {
                let mut m = m.borrow_mut();
                match next {
                    Some(v) => m.insert(self.key, v),
                    None => m.remove(&self.key),
                };
            });
            window.request_animation_frame();
        }
        let extent = match next {
            Some(n) => fit::extent_before_next(n, &entries, top),
            None => fit::extent_before_point(cut.zip(para), &entries, &split, top),
        };
        let prev_extent = crate::text::clamp::clamp_extent(self.key);
        let moved = match (prev_extent, extent) {
            (Some(a), Some(b)) => (a - b).abs() > 0.5,
            (a, b) => a.is_some() != b.is_some(),
        };
        if moved {
            CLAMP_EXTENT.with(|m| {
                let mut m = m.borrow_mut();
                match extent {
                    Some(v) => m.insert(self.key, v),
                    None => m.remove(&self.key),
                };
            });
            window.request_animation_frame();
        }
        let rel = cut.map(|c| (c - top).max(0.0));
        // Гистерезис: мелкие колебания точки (обрезка двигает схлопнутые
        // поля, точка плывёт на доли строки) не перезаписывают её — иначе
        // пары мигали между прогонами. Крупный сдвиг — честный пересчёт.
        // «Измерено: резать нечего» хранится БЕСКОНЕЧНОСТЬЮ, а не пустотой:
        // пустота значит «ещё не мерили», и `styled_div_with` подставлял бы
        // запасной потолок `N × line-height` навсегда (`webkit-line-clamp-029`:
        // все строки в своём контексте, считать нечего, а коробка резалась на
        // три строки из пяти). Запасной потолок остаётся только первому кадру.
        let v = rel.unwrap_or(f32::INFINITY);
        let prev = clamp_cut(self.key);
        let changed = match prev {
            Some(a) if a.is_finite() && v.is_finite() => (a - v).abs() > 4.0,
            Some(a) => a.is_finite() != v.is_finite(),
            None => true,
        };
        if changed {
            CLAMP_CUTS.with(|m| {
                m.borrow_mut().insert(self.key, v);
            });
            window.request_animation_frame();
        }
    }
}
