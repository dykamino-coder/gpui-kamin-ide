//! Painting for cut_element; split out to keep the owning module within 250 lines.

mod automatic;
mod counted;
mod fit;

use super::ClampCut;
use super::{ClampEntry, clamp_cut, clamp_para, take_para_rows};
use crate::text::clamp::line_state::{CLAMP_BOXES, CLAMP_EXTENT, CLAMP_NEXT};
use crate::text::clamp::{CLAMP_CUTS, CLAMP_PARA};
use gpui::{App, Bounds, Pixels, Window};

/// A probed block box inside the clamp container, in box-tree preorder.
#[derive(Debug)]
pub(crate) struct ClampBox {
    /// Border-box top and padding-box bottom.
    pub y0: f32,
    pub y1: f32,
    /// Definite height: not fragmented at the clamp point.
    pub fixed: bool,
    /// End border + padding and end margin.
    pub bp: f32,
    pub margin: f32,
    /// `min-height` border-box extent from `y0`, 0 when none.
    pub min_end: f32,
    pub node: u64,
    /// Paragraph numbers `[first, end)` of its descendants.
    pub span: Option<(u32, u32)>,
}

impl ClampBox {
    /// Inside a monolithic ancestor that ends before `c`: its overflow is
    /// that ancestor's, parked at the ancestor's end, so the box can
    /// neither hold nor push the clamp point.
    pub(super) fn inside_fitting_fixed(&self, all: &[ClampBox], c: f32) -> bool {
        let Some((a, z)) = self.span else {
            return false;
        };
        all.iter().any(|o| {
            !std::ptr::eq(o, self)
                && o.fixed
                && o.y1 <= c + 0.5
                && o.y0 <= self.y0
                && o.span.is_some_and(|(oa, oz)| oa <= a && z <= oz)
        })
    }

    /// Lowest edge the box occupies in the flow: padding box, end border
    /// and padding, end margin.
    pub(super) fn flow_end(&self) -> f32 {
        self.y1 + self.bp + self.margin.max(0.0)
    }
}

/// Where the clamp container's content ends for a clamp point at `point`
/// inside the boxes `chain` (fragmented at it), in preorder (outermost
/// first): end border and padding of each box, `min-height` of each box,
/// end margins collapsing through boxes without border/padding (css-
/// overflow-4 §5.3; Blink `LineClampAncestorChain::
/// InnerFinalLineClampBlockSize`). The clamp container is an independent
/// BFC, so the margin left over at its end stays inside it.
pub(super) fn chain_end(point: f32, chain: &[&ClampBox]) -> f32 {
    chain_end_with(point, chain, true)
}

/// Space the end edges of `chain` take after the point, `min-height` aside
/// (the line budget of `line-clamp: auto`).
pub(super) fn chain_extra(chain: &[&ClampBox]) -> f32 {
    chain_end_with(0.0, chain, false)
}

/// Used border-box heights of the boxes of `chain` (see `chain_end`).
pub(super) fn chain_sizes(point: f32, chain: &[&ClampBox]) -> Vec<(u64, f32)> {
    (0..chain.len())
        .map(|i| {
            let b = chain[i];
            // The chain from the point up to `b`, margins after `b` aside.
            let inner = &chain[i + 1..];
            let mut y = chain_end_with(point, inner, true);
            // Margins pending below `b` collapse through it unless it has
            // end border/padding (then they are inside it).
            if b.bp > 0.0 {
                y += b.bp;
            } else {
                y -= pending_margin(inner);
            }
            let y = if b.min_end > 0.0 {
                y.max(b.y0 + b.min_end)
            } else {
                y
            };
            (b.node, (y - b.y0).max(0.0))
        })
        .filter(|(n, _)| *n != 0)
        .collect()
}

/// End margin still collapsing at the end of `chain` (no border/padding
/// below it).
fn pending_margin(chain: &[&ClampBox]) -> f32 {
    let (mut pos, mut neg) = (0.0f32, 0.0f32);
    for b in chain.iter().rev() {
        if b.bp > 0.0 {
            pos = 0.0;
            neg = 0.0;
        }
        pos = pos.max(b.margin);
        neg = neg.min(b.margin);
    }
    pos + neg
}

fn chain_end_with(point: f32, chain: &[&ClampBox], min: bool) -> f32 {
    let (mut y, mut pos, mut neg) = (point, 0.0f32, 0.0f32);
    for b in chain.iter().rev() {
        if b.bp > 0.0 {
            y += pos + neg + b.bp;
            pos = 0.0;
            neg = 0.0;
        }
        if min && b.min_end > 0.0 && b.y0 + b.min_end > y {
            y = b.y0 + b.min_end;
            pos = 0.0;
            neg = 0.0;
        }
        pos = pos.max(b.margin);
        neg = neg.min(b.margin);
    }
    y + pos + neg
}

impl ClampCut {
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
        // ★ ЗАМЕРЕНО (10.09, `scout-clampmarker-2026-09c.md`, 21 хунк):
        // срез 416 пар (семья `line-clamp` + схлопывание полей) 259 ->
        // 260. Взяты `line-clamp-auto-003` 1.82 -> 0.00 и `-047`
        // 1.62 -> 0.12; четвёрка `-018`…`-021` стояла на 0.39-0.40 и
        // встала РОВНО на 0.00 — знак наконец в конце текста строки.
        // Потеря одна: `-022` 0.39 -> 2.61 — тот же тест, что `-021`,
        // но `max-height: 5.5lh` вместо `5lh` при том же эталоне;
        // полстроки сверх бюджета у нас пускают лишнюю строку. Долг
        // отдельным подкорнем CLAMP-HALF-LINE.
        // Знак обрыва в АВТО-режиме (`limit == None`). Рисует его НЕ этот
        // слой: сюда возвращается только БЮДЖЕТ строк — номер абзаца в
        // контейнере и сколько его строк остаётся выше среза, — а «…»
        // ставит уже проверенный `lines::clamp_lines`/`paint_line`: в
        // конце ТЕКСТА строки, с выключкой и направлением письма
        // (css-overflow-4: знак «is placed at the end of the line box
        // reducing the space available to the other contents of the
        // line», а для bidi — анонимный строчный с уровнем bidi-абзаца).
        // Так же развязан и Blink: блочный слой отдаёт признак
        // `IsAtClampPoint`, а ширину знака получает разрыватель строк
        // (`inline_layout_algorithm.cc:1247` `SetupLineClampEllipsis` →
        // `SetLineClampEllipsisWidth`). Прежний набросок рисовал знак у
        // ПРАВОГО края коробки — мимо конца текста, мимо выключки и
        // мимо rtl.
        // Числовой предел, который `max-height` перехватил раньше N-й строки
        // (`line-clamp: 4 auto` при `max-height: 3lh`), — та же точка
        // обрыва, что в авто-режиме: §5.3 берёт ПЕРВУЮ из двух точек, и знак
        // встаёт на последнюю строку перед ней (`line-clamp-041`).
        let para = cut.filter(|_| !by_count).and_then(|c| {
            // Есть ли что резать. Как только бюджет применён, абзац УЖЕ
            // укорочен и сам за срез не выходит — признак защёлкивается
            // применённым бюджетом, иначе кадры зациклились бы:
            // обрезали → влезло → сняли → снова не влезло.
            let overflow = entries.iter().any(|e| e.clamped.is_some())
                || entries.iter().any(|e| {
                    f32::from(e.bounds.origin.y) + f32::from(e.bounds.size.height) > c + 0.5
                });
            if !overflow {
                return None;
            }
            // Знак садится на ПОСЛЕДНЮЮ строку перед точкой среза — в том
            // числе когда точка стоит МЕЖДУ блоками и сам абзац видим
            // целиком. Абзацы в своём контексте форматирования
            // пропускаются: точкой среза их строки быть не могут.
            // Последняя строка перед точкой — во ВЛОЖЕННОМ контексте
            // форматирования (несчитаемая): знак не ставится вовсе, а не
            // уходит на предыдущий считаемый абзац (css-overflow-4 §5.3:
            // многоточие — на последней строке ПЕРЕД точкой среза в этом
            // BFC; `line-clamp-auto-034/039`: «Line 4» без знака).
            entries
                .iter()
                .filter(|e| e.line > 0.0 && (e.skip_count || e.seq.is_some()))
                .filter_map(|e| {
                    let seq = if e.skip_count { None } else { e.seq };
                    let h = f32::from(e.bounds.size.height);
                    if h <= 0.0 {
                        return None;
                    }
                    let rows = fit(e);
                    // Точка `c` уже стоит на СРЕЗАННОМ низу последней
                    // строки — полный низ этой строки ниже на `trim_end`.
                    let k = rows
                        .iter()
                        .filter(|(_, b)| *b <= c + trim_end + 0.5)
                        .count();
                    (k >= 1).then(|| (rows[k - 1].1, seq, k))
                })
                // The clamp point follows the LAST fitting line in box-tree
                // order: on equal bottoms the later paragraph wins
                // (`line-clamp-auto-043`: a line beside an earlier block's
                // overflow).
                .max_by(|a, b| {
                    if (a.0 - b.0).abs() <= 0.5 {
                        a.1.cmp(&b.1)
                    } else {
                        a.0.total_cmp(&b.0)
                    }
                })
                // Пустая блочная коробка между последней строкой и точкой
                // среза: точка — после неё (последняя возможная), и строка
                // точке уже не предшествует — знака нет (§5.3).
                // Not when that line is where WE cut the paragraph: the
                // empty box then sits right after the cut only because the
                // clamped paragraph got shorter, and the point stays in the
                // paragraph (`line-clamp-with-abspos-021`,
                // `-with-fixed-pos-016`).
                .filter(|(bottom, seq, _)| {
                    seq.is_some_and(|s| {
                        entries
                            .iter()
                            .any(|e| e.seq == Some(s) && e.clamped.is_some())
                    }) || !empties.iter().any(|e| {
                        let y0 = f32::from(e.bounds.origin.y);
                        e.empty
                            && f32::from(e.bounds.size.height) <= 0.5
                            && y0 >= *bottom - 0.5
                            && y0 <= c + 0.5
                    })
                })
                .and_then(|(_, seq, k)| seq.map(|s| (s, k)))
        });
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
