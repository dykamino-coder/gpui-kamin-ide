//! Painting for cut_element; split out to keep the owning module within 250 lines.

mod automatic;
mod counted;

use super::ClampCut;
use super::{ClampEntry, clamp_cut, clamp_para, take_para_rows};
use crate::text::clamp::{CLAMP_CUTS, CLAMP_PARA};
use gpui::{App, Bounds, GlobalElementId, InspectorElementId, Pixels, Window};

impl ClampCut {
    pub(crate) fn paint_impl(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
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
        let entries: Vec<ClampEntry> = all_entries.into_iter().filter(|e| !e.empty).collect();
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
        // (верх, низ, заданная высота, нижние рамка+паддинг)
        let mut blocks: Vec<(f32, f32, bool, f32)> = vec![];
        for e in &entries {
            let y0 = f32::from(e.bounds.origin.y);
            let h = f32::from(e.bounds.size.height);
            if e.line > 0.0 && h > 0.0 {
                for (a, b) in split(e) {
                    rows.push((a, b, !e.skip_count));
                }
            } else if h > 0.0 {
                blocks.push((y0, y0 + h, e.fixed_height, e.bp_after));
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
        let (counted_cut, num_para, by_count) = self.counted_cut(cut, &entries, &blocks, &split);
        cut = counted_cut;
        cut = self.automatic_cut(cut, by_count, &blocks, &rows, trim_end);
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
                    let rows = split(e);
                    // Точка `c` уже стоит на СРЕЗАННОМ низу последней
                    // строки — полный низ этой строки ниже на `trim_end`.
                    let k = rows
                        .iter()
                        .filter(|(_, b)| *b <= c + trim_end + 0.5)
                        .count();
                    (k >= 1).then(|| (rows[k - 1].1, seq, k))
                })
                .max_by(|a, b| a.0.total_cmp(&b.0))
                // Пустая блочная коробка между последней строкой и точкой
                // среза: точка — после неё (последняя возможная), и строка
                // точке уже не предшествует — знака нет (§5.3).
                .filter(|(bottom, _, _)| {
                    !empties.iter().any(|e| {
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
