//! Выключка при отрисовке (paint_justified): слова, полосы, видимые прогоны.

use crate::text::paragraph::*;
use gpui::{App, Bounds, Hsla, Pixels, SharedString, TextRun, Window, point, px};

impl Paragraph {
    /// Выключка по ширине: остаток строки раздаётся её пробелам.
    ///
    /// Слова набираются по отдельности и ставятся каждое на своё место —
    /// иначе растянуть промежутки нечем: набор отдаёт готовую строку одним
    /// куском. Внутри слова набор остаётся сплошным, поэтому лигатуры и вязь
    /// не рвутся.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_justified(
        &self,
        range: &std::ops::Range<usize>,
        segs: &[Seg],
        free: Pixels,
        line_width: Pixels,
        bounds: Bounds<Pixels>,
        y: Pixels,
        pad: (f32, f32),
        dx: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut words = self.words(range);
        // Слово режется по границам кусков с трекингом: набор принимает
        // трекинг скаляром, поэтому кусок с другим значением обязан идти
        // отдельным вызовом. Без этого `letter-spacing` на `<span>` внутри
        // слова не действовал вовсе.
        if !self.letter_spans.is_empty()
            || !self.shift_spans.is_empty()
            || !self.rel_spans.is_empty()
            || !self.edge_spans.is_empty()
        {
            let mut cuts: Vec<usize> = Vec::new();
            let mut cut = |edge: usize| {
                if edge > range.start && edge < range.end {
                    cuts.push(edge);
                }
            };
            for (r, _) in self.letter_spans.iter() {
                // Трекинг знака — это промежуток ПОСЛЕ него, поэтому у
                // последнего знака отрезка он на набор внутри отрезка не
                // влияет. Резать по началу нужно только там, где знаков в
                // диапазоне несколько: одиночный (зазор `text-autospace`)
                // спокойно доживает в общем отрезке, а свой разрез оставлял
                // между половинками слова шов в точку — соседние отрезки
                // округляются независимо (`text-autospace-001`: `XX`
                // расходились).
                // A box spacer carries its entire advance in tracking (CSS 2.1
                // section 8.3). Joining it to a preceding word discards that
                // advance when the word is shaped with its own spacing.
                if self.text[r.clone()].chars().nth(1).is_some()
                    || self.spacers.binary_search(&r.start).is_ok()
                {
                    cut(r.start);
                }
                cut(r.end);
            }
            // Сдвиг по вертикали — свойство самого глифа: он обязан ехать
            // отдельным вызовом целиком.
            for (r, _) in self.rel_spans.iter() {
                cut(r.start);
                cut(r.end);
            }
            for (r, _) in self.shift_spans.iter() {
                cut(r.start);
                cut(r.end);
            }
            for (r, _, _) in self.edge_spans.iter() {
                cut(r.start);
                cut(r.end);
            }
            cuts.sort_unstable();
            cuts.dedup();
            let mut split: Vec<Word> = Vec::with_capacity(words.len());
            for w in words {
                let mut at = w.range.start;
                let mut spaces = w.spaces_before;
                for cut in cuts.iter().copied().filter(|c| w.range.contains(c)) {
                    split.push(Word {
                        range: at..cut,
                        spaces_before: spaces,
                    });
                    at = cut;
                    spaces = 0;
                }
                split.push(Word {
                    range: at..w.range.end,
                    spaces_before: spaces,
                });
            }
            words = split;
        }
        // UAX #9 L2 in a left-to-right line: the word path shapes and places
        // pieces one by one, so right-to-left level runs (explicit controls,
        // `unicode-bidi`, strong R/AL text) must be reordered here; without it
        // they were painted in logical order (CSS 2.1 §9.10, `bidi-text/*`).
        // Runs come in visual order; a word crossing a run edge is cut there.
        let ltr_runs = self.line_visual_runs(range);
        if !ltr_runs.is_empty() {
            let mut split: Vec<Word> = Vec::with_capacity(words.len());
            for w in words {
                let mut at = w.range.start;
                let mut edges: Vec<usize> = ltr_runs
                    .iter()
                    .flat_map(|r| [r.0, r.1])
                    .chain(
                        self.spacers
                            .iter()
                            .flat_map(|p| [*p, *p + crate::text::inline::SPACER.len()]),
                    )
                    .chain(self.box_extents.iter().flat_map(|b| [b.1, b.2]))
                    .filter(|s| *s > at && *s < w.range.end)
                    .collect();
                edges.sort_unstable();
                edges.dedup();
                for c in edges {
                    split.push(Word {
                        range: at..c,
                        spaces_before: w.spaces_before,
                    });
                    at = c;
                }
                split.push(Word {
                    range: at..w.range.end,
                    spaces_before: w.spaces_before,
                });
            }
            words = split;
        }
        // Растягивается КАЖДЫЙ пробел, а не промежуток между словами: там, где
        // подряд стоят два сохранённых пробела, добавка идёт дважды.
        //
        // Пробелы ЛЕВЕЕ последней табуляции добавки не получают: табуляция
        // доводит строку до своей позиции и всё лишнее место слева от себя
        // поглощает, поэтому позиции табуляции совпадают с нерастянутой
        // строкой (css-text-4 §8.1). Отсюда оба поведения сразу: строка, где
        // все пробелы левее табуляции, не растягивается вовсе
        // (`text-align-justify-tabs-001`, обе коробки обязаны совпасть), а
        // остаток достаётся только пробелам правее (`-002`: их ровно два, и
        // каждый вырастает на пробел).
        let absorbed = self.text[range.clone()]
            .rfind('\u{9}')
            .map_or(0, |at| self.justify_opps(range, range.start + at));
        if absorbed > 0 {
            for w in words.iter_mut() {
                w.spaces_before = w.spaces_before.saturating_sub(absorbed);
            }
        }
        let opportunities = words.last().map(|w| w.spaces_before).unwrap_or(0);
        let step = if opportunities > 0 {
            free / opportunities as f32
        } else {
            px(0.)
        };
        let from = self.x_at(segs, range.start, Edge::Start);
        // Ось зеркала правой строки — её СОБСТВЕННЫЙ правый край, а не край
        // коробки: прижим уже учтён в `dx`, и вычитать его из ширины коробки
        // значит ошибиться на `free − 2·dx` (`bidi-box-model-013`: 380 точек).
        // ★ ЗАМЕРЕНО, ЭФФЕКТА НЕТ (01.09): зажимать ось шириной коробки
        // (`line_width.min(bounds.size.width)`), чтобы переполняющая строка в
        // rtl росла ВЛЕВО, как это делают блоки. Срез из 157 пар rtl/bidi —
        // 139 → 139, `absolute-non-replaced-width-021/022/023` остались 0.85.
        // Зажим не срабатывает: `bounds` здесь — коробка САМОГО абзаца, и она
        // уже растянута по содержимому. Корень выше: абзац не зажат
        // `max-width` родителя, а не ось зеркала.
        let mirror = bounds.origin.x + dx + line_width + free;
        // UAX#9 L2 разворачивает прогоны уровня ≥1, а зеркало строки
        // переворачивает ВСЕ слова разом: латинский прогон внутри правого
        // абзаца выходил задом наперёд. Прогоны левого уровня выкладываем
        // заново — в своём порядке и на своём месте (выключенный путь
        // `BidiInfo` не считал вовсе).
        //
        // ЗАМЕРЕНО: приобретено 1, потерь нет. Ожидалось больше: в семье
        // `bidi-box-model-*` левый прогон чаще всего ОДНО слово, а одиночное
        // слово зеркало кладёт верно и без разбора. Остаток той семьи держат
        // распорки полей, а не порядок слов.
        // ПРОБОВАЛИ И ОТКАТИЛИ: распространить разворот и на ЛЕВЫЕ строки
        // (правый прогон внутри левого абзаца). Замерено: 5063 -> 5059, четыре
        // пары `CSS2/bidi-005..009` перешли порог 0.50 -> 0.51. Прогоны там
        // однознаковые, и разворот нужен ПОГЛИФНЫЙ, а наш идёт по словам.
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.10): поглифный вариант — прогоны UAX#9 L2
        // левой строки в видимом порядке, слово правого уровня набором
        // `shape_line_rtl`, в правой строке слово правого уровня тоже
        // справа налево и место без трекинга хвоста, плюс накопительный
        // счёт пробелов у разрезанного трекингом слова. Тестовые половины
        // `letter-spacing-bidi-003` встали верно, но эталон держит флоат,
        // уходящий строкой ниже, — пара красная. Срез 845 пар (bidi,
        // letter-spacing, text-justify, text-align, word-spacing, CSS2/text):
        // 779 -> 779, `bidi-011` 1.05 -> 2.26 (распорки полей `<span>` с RLO
        // остаются на логическом месте). Возвращать вместе с распорками,
        // переставляемыми по прогонам (патч: `target/perword-bidi-2026-10-04.patch`).
        // Logical offset of a byte from the line start, with the justification
        // added by the separators before it (same count as `words`).
        let logical_at = |pos: usize| -> Pixels {
            let seps = self.justify_opps(range, pos).saturating_sub(absorbed);
            (self.x_at(segs, pos, Edge::Start) - from) + step * seps as f32
        };
        // Visual pieces of the line, left to right: each level run in visual
        // order (UAX #9 L2), cut at box edge spacers, with its logical extent.
        let mut ltr_place: Vec<(usize, usize, bool, Pixels)> = Vec::new();
        if !ltr_runs.is_empty() {
            let mut units: Vec<(usize, usize, bool)> = Vec::new();
            for &(s, e, rtl) in ltr_runs.iter() {
                let mut pts = vec![s, e];
                for &p in self.spacers.iter().filter(|p| **p >= s && **p < e) {
                    pts.push(p);
                    pts.push((p + crate::text::inline::SPACER.len()).min(e));
                }
                // Box content edges: a fragment of a box starts a piece.
                for b in &self.box_extents {
                    pts.extend([b.1, b.2].into_iter().filter(|p| *p > s && *p < e));
                }
                pts.sort_unstable();
                pts.dedup();
                let mut run: Vec<(usize, usize, bool)> =
                    pts.windows(2).map(|w| (w[0], w[1], rtl)).collect();
                if rtl {
                    run.reverse();
                }
                units.extend(run);
            }
            // CSS 2.1 §8.6 / Blink `UpdateFragmentEdges`: a box split by
            // reordering keeps its left edge on its leftmost fragment and its
            // right edge on the rightmost one. Inner boxes first.
            let mut boxes: Vec<(u32, usize, usize)> = Vec::new();
            for &(p, id, _, _) in &self.spacer_edges {
                match boxes.iter_mut().find(|b| b.0 == id) {
                    Some(b) => {
                        b.1 = b.1.min(p);
                        b.2 = b.2.max(p);
                    }
                    None => boxes.push((id, p, p)),
                }
            }
            for b in boxes.iter_mut() {
                if let Some(x) = self.box_extents.iter().find(|x| x.0 == b.0) {
                    b.1 = b.1.min(x.1);
                    b.2 = b.2.max(x.2);
                }
            }
            boxes.sort_by_key(|b| b.2 - b.1);
            for &(id, lo, hi) in &boxes {
                let own = |a: usize| {
                    self.spacer_edges
                        .iter()
                        .find(|e| e.0 == a && e.1 == id)
                        .map(|e| (e.2, e.3))
                };
                if !units.iter().any(|u| own(u.0).is_some()) {
                    continue;
                }
                let rest: Vec<(usize, usize, bool)> = units
                    .iter()
                    .copied()
                    .filter(|u| own(u.0).is_none())
                    .collect();
                // Content of the box: between its markers when known, else
                // strictly between its own edge spacers.
                let (from_at, to_at) = self
                    .box_extents
                    .iter()
                    .find(|b| b.0 == id)
                    .map_or((lo + 1, hi), |b| (b.1, b.2));
                let inside: Vec<usize> = rest
                    .iter()
                    .enumerate()
                    .filter(|(_, u)| u.0 >= from_at && u.0 < to_at && u.0 < u.1)
                    .map(|(i, _)| i)
                    .collect();
                let (Some(&first), Some(&last)) = (inside.first(), inside.last()) else {
                    continue;
                };
                // Outermost spacer (the margin) goes furthest out: logical
                // order already is visual for an ltr parent, reversed for rtl.
                let mut lefts: Vec<(usize, usize, bool)> = Vec::new();
                let mut rights: Vec<(usize, usize, bool)> = Vec::new();
                let mut parent_rtl = false;
                for &(a, b, rtl) in units.iter() {
                    if let Some((left, prtl)) = own(a) {
                        parent_rtl = prtl;
                        if left {
                            lefts.push((a, b, rtl));
                        } else {
                            rights.push((a, b, rtl));
                        }
                    }
                }
                lefts.sort_by_key(|u| u.0);
                rights.sort_by_key(|u| u.0);
                if parent_rtl {
                    lefts.reverse();
                    rights.reverse();
                }
                let mut next = rest;
                next.splice(last + 1..last + 1, rights);
                next.splice(first..first, lefts);
                units = next;
            }
            let mut cursor = px(0.);
            for (a, b, rtl) in units {
                ltr_place.push((a, b, rtl, cursor));
                cursor += logical_at(b) - logical_at(a);
            }
        }
        // Visual offset (from the line start) of the logical piece `a..b`: a
        // right-to-left piece mirrors its contents within its own extent.
        let placed = |a: usize, b: usize| -> Option<(Pixels, bool)> {
            let &(s, e, rtl, start) = ltr_place.iter().find(|p| p.0 <= a && a < p.1)?;
            Some(if rtl {
                (start + (logical_at(e) - logical_at(b.min(e))), true)
            } else {
                (start + (logical_at(a) - logical_at(s)), false)
            })
        };
        let mut logical_run: Vec<(usize, Pixels)> = vec![];
        if self.wrap.rtl
            && ltr_place.is_empty()
            && range.start < range.end
            && range.end <= self.text.len()
        {
            let info = unicode_bidi::BidiInfo::new(&self.text, Some(unicode_bidi::Level::rtl()));
            if let Some(para) = info
                .paragraphs
                .iter()
                .find(|p| p.range.start <= range.start && range.start < p.range.end)
                .or_else(|| info.paragraphs.first())
            {
                let (levels, visual) = info.visual_runs(para, range.clone());
                for run in visual {
                    if levels.get(run.start).is_some_and(|l| l.is_rtl()) {
                        continue;
                    }
                    let idx: Vec<usize> = words
                        .iter()
                        .enumerate()
                        .filter(|(_, w)| w.range.start >= run.start && w.range.start < run.end)
                        .map(|(i, _)| i)
                        .collect();
                    if idx.len() < 2 {
                        continue;
                    }
                    // Зеркальные места прогона: слева лежит ПОСЛЕДНЕЕ слово.
                    let mut mirrored: Vec<(usize, Pixels, Pixels)> = idx
                        .iter()
                        .map(|&i| {
                            let w = &words[i];
                            let width = self.word_width(w, window);
                            let logical = (self.x_at(segs, w.range.start, Edge::Start) - from)
                                + step * w.spaces_before as f32;
                            let x = mirror - logical - width;
                            (i, x, width)
                        })
                        .collect();
                    mirrored
                        .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                    // Промежутки между зеркальными соседями: в логическом
                    // порядке те же самые, только в обратную сторону.
                    let gaps: Vec<Pixels> = mirrored
                        .windows(2)
                        .map(|p| p[1].1 - (p[0].1 + p[0].2))
                        .rev()
                        .collect();
                    let mut cursor = mirrored[0].1;
                    for (k, &i) in idx.iter().enumerate() {
                        let width = mirrored
                            .iter()
                            .find(|(j, _, _)| *j == i)
                            .map(|(_, _, w)| *w)
                            .unwrap_or(px(0.));
                        logical_run.push((i, cursor));
                        cursor += width + gaps.get(k).copied().unwrap_or(px(0.));
                    }
                }
            }
        }
        let line_base = self.base_of(range);
        let mut decor_prev: Option<(usize, Pixels, Pixels, Pixels)> = None;
        for (wi, word) in words.iter().enumerate() {
            let slice: SharedString = self.text[word.range.clone()].to_string().into();
            // Полоса строчной коробки продолжается сквозь слова (см.
            // `slice_runs_banded`); при rtl слова зеркалятся, и стороны
            // меняются местами — там прежний счёт.
            let mut runs = match ltr_place
                .iter()
                .position(|p| p.0 <= word.range.start && word.range.start < p.1)
            {
                Some(k) => self.visual_band_runs(&ltr_place, k, &word.range),
                None if self.wrap.rtl => self.mirrored_band_runs(&word.range),
                None => slice_runs_banded(&self.runs, &word.range),
            };
            let decor = self.decor_on();
            if decor {
                for r in runs.iter_mut() {
                    r.underline = None;
                    r.strikethrough = None;
                }
            }
            let spacing = self
                .letter_spans
                .iter()
                .find(|(r, _)| r.contains(&word.range.start))
                .map(|(_, v)| *v)
                .unwrap_or(self.letter_spacing);
            let visual = placed(word.range.start, word.range.end);
            let shaped = if visual.is_some_and(|v| v.1) {
                // A right-to-left level piece: glyphs in visual order (UAX #9
                // L2/L4), shaped once under an RLO like `shape`.
                let mut runs = runs;
                let body = controlled_shape::text(&slice, &mut runs, true);
                window
                    .text_system()
                    .with_ligature_breaking(false)
                    .shape_line_rtl(body, self.font_size, &runs, spacing)
            } else {
                window
                    .text_system()
                    .with_ligature_breaking(false)
                    .shape_line_spaced(slice, self.font_size, &runs, None, spacing)
            };
            let logical = (self.x_at(segs, word.range.start, Edge::Start) - from)
                + step * word.spaces_before as f32;
            // При письме справа налево строка раздаётся от ПРАВОГО края:
            // первое слово встаёт справа, последнее — слева. Раздача слева
            // направо переворачивала порядок слов на выключенной строке.
            let x = match logical_run.iter().find(|(i, _)| *i == wi) {
                Some((_, fixed)) => *fixed,
                // A right-to-left line starts at its mirror axis: its
                // logical extent ends at the visual left.
                None if visual.is_some() => {
                    let left = if self.wrap.rtl {
                        mirror - logical_at(range.end)
                    } else {
                        bounds.origin.x + dx
                    };
                    left + visual.map_or(px(0.), |v| v.0)
                }
                None if self.wrap.rtl => mirror - logical - shaped.width,
                None => bounds.origin.x + dx + logical,
            };
            // Сдвиг куска по вертикали: надстрочный и подстрочный знак стоят
            // выше и ниже базовой линии, оставаясь в той же строке.
            let dy = self
                .shift_spans
                .iter()
                .find(|(r, _)| r.contains(&word.range.start))
                .map(|(_, v)| *v)
                .unwrap_or(px(0.));
            // Относительный сдвиг двигает ТОЛЬКО отрисовку куска: место в
            // потоке за ним сохраняется, соседи не съезжают (§9.4.3).
            let (rx, ry) = self
                .rel_spans
                .iter()
                .find(|(r, _)| r.contains(&word.range.start))
                .map(|(_, v)| *v)
                .unwrap_or((0.0, 0.0));
            // Слово набирается своим вызовом, и `ShapedLine::paint` ставит
            // его базовую линию по СВОИМ подъёму и спуску. Сплошной набор
            // строки берёт наибольшие по всей строке — куски разного кегля
            // стоят на одной базовой линии (§10.8). Слово опускается на
            // разницу: у строки из одного шрифта она ровно ноль.
            let fix = match (line_base, self.base_of(&word.range)) {
                (Some(l), Some(w)) => px(l - w),
                _ => px(0.),
            };
            // Кусок у края строки: его строчная коробка (высотой своей
            // `line-height`, глифы по её полулидингу) встаёт верхом на верх
            // строки или низом на низ (§10.8.1). `shaped.paint` центрирует
            // глифы в высоте строки абзаца — разница высот делится пополам.
            let fix = match self
                .edge_spans
                .iter()
                .find(|(r, _, _)| r.contains(&word.range.start))
            {
                Some((_, top, h)) => {
                    let lh = f32::from(self.line_height);
                    let line_top = -pad.0;
                    let box_top = if *top { line_top } else { lh + pad.1 - h };
                    px(box_top + (h - lh) / 2.0)
                }
                None => fix,
            };
            let at = point(x + px(rx), y + dy + px(ry) + fix);
            // Подложка прогона — отдельным вызовом, см. выше.
            self.paint_run_background(&shaped, at, window, cx);
            let origin = self.text_raster_origin(&shaped, at, window);
            if decor {
                // Украшения промежутка до слова: от правого края прошлого
                // слова до левого края этого (растянутый пробел тоже
                // украшается, css-text-decor-3 §2.1).
                if let Some((end, right, base, pdy)) = decor_prev
                    && end < word.range.start
                    && !self.wrap.rtl
                {
                    let left = at.x;
                    let gap = end..word.range.start;
                    let x_of = |_: usize, _: usize| (right, left);
                    self.paint_decor(gap.clone(), &x_of, base, pdy, range, false, window);
                    self.paint_decor(gap.clone(), &x_of, base, pdy, range, true, window);
                }
                self.paint_decor_shaped(&word.range, &shaped, at, dy, false, range, false, window);
            }
            let _ = shaped.paint(
                origin,
                self.line_height,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            );
            if decor {
                self.paint_decor_shaped(&word.range, &shaped, at, dy, false, range, true, window);
                let base = at.y
                    + (self.line_height - shaped.ascent - shaped.descent) / 2.0
                    + shaped.ascent;
                decor_prev = Some((word.range.end, at.x + shaped.width, base, dy));
            }
            // Пробелы между словами тоже принадлежат полосе коробки: без
            // этого фон и рамка `<span>` рвались на каждом пробеле. Промежуток
            // набирается своими прогонами (обе стороны — продолжение полосы) и
            // красит только подложку. Растянутые выключкой промежутки красит
            // ветка ниже.
            if step == px(0.)
                && !self.wrap.rtl
                && let Some(next) = words.get(wi + 1)
                && next.range.start > word.range.end
            {
                let gap = word.range.end..next.range.start;
                let gap_runs = slice_runs_banded(&self.runs, &gap);
                if gap_runs.iter().any(|r| r.background_color.is_some()) {
                    let gap_text: SharedString = self.text[gap.clone()].to_string().into();
                    let spacing = self
                        .letter_spans
                        .iter()
                        .find(|(r, _)| r.contains(&gap.start))
                        .map(|(_, v)| *v)
                        .unwrap_or(self.letter_spacing);
                    let mut gap_shaped = window
                        .text_system()
                        .with_ligature_breaking(false)
                        .shape_line_spaced(
                            gap_text.clone(),
                            self.font_size,
                            &gap_runs,
                            None,
                            spacing,
                        );
                    // Ширина промежутка — по РАЗЛОЖЕННОЙ строке: там в нём уже
                    // лежит `word-spacing`, а отдельный набор пробела его не
                    // знает, и полоса `<span>` рвалась на каждом растянутом
                    // пробеле (`word-spacing-characters-001`). Недостача
                    // раздаётся трекингом по знакам промежутка.
                    let want = self.x_at(segs, gap.end, Edge::Start)
                        - self.x_at(segs, gap.start, Edge::Start);
                    let n = gap_text.chars().count().max(1) as f32;
                    if (want - gap_shaped.width).abs() > px(0.5) {
                        gap_shaped = window
                            .text_system()
                            .with_ligature_breaking(false)
                            .shape_line_spaced(
                                gap_text,
                                self.font_size,
                                &gap_runs,
                                None,
                                spacing + (want - gap_shaped.width) / n,
                            );
                    }
                    let gap_x = bounds.origin.x
                        + dx
                        + placed(gap.start, gap.end)
                            .map_or(self.x_at(segs, gap.start, Edge::Start) - from, |v| v.0);
                    let gap_y = match (line_base, self.base_of(&gap)) {
                        (Some(l), Some(w)) => y + px(l - w),
                        _ => y,
                    };
                    self.paint_run_background(&gap_shaped, point(gap_x, gap_y), window, cx);
                }
            }
            // Растянутый выключкой пробел тоже принадлежит прогону, и его
            // подложка обязана быть сплошной. Красим ТОЛЬКО когда пробел
            // целиком внутри одного прогона с фоном — иначе фон соседнего
            // куска растекается по чужому месту (замерено: css-text 1015 →
            // 691 при покраске каждого промежутка).
            if step > px(0.)
                && !self.wrap.rtl
                && ltr_place.is_empty()
                && let Some(next) = words.get(wi + 1)
                && next.range.start > word.range.end
                && let Some(bg) = self.gap_background(word, next)
            {
                let after = (self.x_at(segs, next.range.start, Edge::Start) - from)
                    + step * next.spaces_before as f32;
                let right = bounds.origin.x + dx + after;
                let left = x + shaped.width;
                if right > left {
                    window.paint_quad(gpui::fill(
                        Bounds {
                            origin: point(left, y + dy),
                            size: gpui::size(right - left, self.line_height),
                        },
                        bg,
                    ));
                }
            }
        }
        if self.decor_on() {
            self.flush_decor(window);
        }
    }

    /// Ширина слова в наборе — тем же путём, что и отрисовка.
    /// ПРОБОВАЛИ И ОТКАТИЛИ: чистить слово от незримых знаков перед набором
    /// и замером, как это делает общий путь (`trim_runs`). Проба по 296 парам
    /// семей `bidi-*`, `letter-spacing-*`, `shaping-arabic-*`: не сдвинулась
    /// НИ ОДНА — знаки управления двунаправленностью лежат в своих кусках, а
    /// не внутри слов, и в этот путь не попадают.
    pub(crate) fn word_width(&self, word: &Word, window: &mut Window) -> Pixels {
        let slice: SharedString = self.text[word.range.clone()].to_string().into();
        let runs = slice_runs(&self.runs, &word.range);
        window
            .text_system()
            .with_ligature_breaking(false)
            .shape_line_spaced(
                slice,
                self.font_size,
                &runs,
                None,
                self.letter_spans
                    .iter()
                    .find(|(r, _)| r.contains(&word.range.start))
                    .map(|(_, v)| *v)
                    .unwrap_or(self.letter_spacing),
            )
            .width
    }

    /// Подложка промежутка между словами — только если оба соседа и сам
    /// промежуток лежат в ОДНОМ прогоне, и у него есть фон.
    pub(crate) fn gap_background(&self, left: &Word, right: &Word) -> Option<gpui::Hsla> {
        let run_at = |at: usize| -> Option<usize> {
            let mut start = 0usize;
            for (i, run) in self.runs.iter().enumerate() {
                if at < start + run.len {
                    return Some(i);
                }
                start += run.len;
            }
            None
        };
        let a = run_at(left.range.end.saturating_sub(1))?;
        let b = run_at(right.range.start)?;
        let gap = run_at(left.range.end)?;
        if a != b || a != gap {
            return None;
        }
        self.runs[a].background_color
    }

    /// Band identity (colour and border) of the run holding byte `at`.
    pub(crate) fn band_at(&self, at: usize) -> Option<(Option<Hsla>, Option<(Hsla, [Pixels; 4])>)> {
        let mut start = 0usize;
        for run in self.runs.iter() {
            if at < start + run.len {
                return Some((run.background_color, run.background_border));
            }
            start += run.len;
        }
        None
    }

    /// Runs of a word of a mirrored right-to-left line (no reordered
    /// pieces): the word stands as one unit, its logical successor on its
    /// left and its predecessor on its right. The inline box band goes on
    /// across a side whose neighbour belongs to the same band, and that side
    /// gets no padding or border (css-break-3 §5.4 `box-decoration-break:
    /// slice`; mirror of `slice_runs_banded`).
    pub(crate) fn mirrored_band_runs(&self, word: &std::ops::Range<usize>) -> Vec<TextRun> {
        let left = self.band_at(word.end);
        let right = word.start.checked_sub(1).and_then(|a| self.band_at(a));
        let mut out = slice_runs(&self.runs, word);
        for run in out.iter_mut() {
            if run.background_color.is_none() {
                continue;
            }
            let own = Some((run.background_color, run.background_border));
            cut_band_sides(run, left == own, right == own);
        }
        out
    }

    /// Runs of a word placed by visual pieces (`ltr_place`, piece `k`): an
    /// inline box draws a side (padding and border) only where its own edge
    /// spacer is the VISUAL neighbour. A box split by bidi reordering keeps
    /// its left edge on its leftmost fragment and its right edge on the
    /// rightmost one, where the line painter put its spacers; a fragment
    /// continued from or onto another line has no side there (CSS 2.1 §8.6,
    /// css-break-3 §5.4; Blink `NGInlineBoxFragmentPainter` paints sides
    /// per `NGPhysicalBoxFragment::SidesToInclude`).
    pub(crate) fn visual_band_runs(
        &self,
        place: &[(usize, usize, bool, Pixels)],
        k: usize,
        word: &std::ops::Range<usize>,
    ) -> Vec<TextRun> {
        let rtl = place[k].2;
        let vis = self.visual_chars(place.iter().map(|p| (p.0, p.1, p.2)));
        let (left, right) = visual_neighbours(&self.text, &vis, word);
        let mut out = slice_runs(&self.runs, word);
        self.cut_piece_sides(&mut out, word, rtl, left, right);
        out
    }

    /// Byte offsets of the characters of a line in visual order, from its
    /// pieces `(start, end, rtl)` in visual order.
    pub(crate) fn visual_chars(
        &self,
        pieces: impl Iterator<Item = (usize, usize, bool)>,
    ) -> Vec<usize> {
        let mut out = Vec::new();
        for (s, e, rtl) in pieces {
            let Some(text) = self.text.get(s..e) else {
                continue;
            };
            let at = out.len();
            out.extend(text.char_indices().map(|(i, _)| s + i));
            if rtl {
                out[at..].reverse();
            }
        }
        out
    }

    /// Cut the band sides at the visual edges of a piece `range` (its runs
    /// `out`, in logical order; `rtl` when its glyphs run right to left)
    /// whose visual neighbours are the bytes `left` and `right`.
    ///
    /// A box with edge spacers (`box_extents`) draws a side only next to its
    /// own spacer: the spacer is where its padding and border sit, so any
    /// other neighbour — another fragment of a box split by bidi reordering,
    /// a segment separator, the start or end of a continued line — means the
    /// box goes on (CSS 2.1 §8.6, css-break-3 §5.4 `box-decoration-break:
    /// slice`). A band without spacers (an outline) goes on only into the
    /// same band, as in `slice_runs_banded`.
    pub(crate) fn cut_piece_sides(
        &self,
        out: &mut [TextRun],
        range: &std::ops::Range<usize>,
        rtl: bool,
        left: Option<usize>,
        right: Option<usize>,
    ) {
        let n = out.len();
        if n == 0 {
            return;
        }
        // Byte offsets of the runs in `out`.
        let mut starts = Vec::with_capacity(n);
        let mut at = range.start;
        for r in out.iter() {
            starts.push(at);
            at += r.len;
        }
        let (li, ri) = if rtl { (n - 1, 0) } else { (0, n - 1) };
        // A piece of zero-width bidi controls only (the marks of `direction`
        // and `unicode-bidi`) has no extent of its own to frame.
        let ghost = self
            .text
            .get(range.clone())
            .is_some_and(|t| t.chars().all(bidi_control));
        let mut cuts: Vec<(usize, bool)> = Vec::new();
        for (idx, nb, is_left) in [(li, left, true), (ri, right, false)] {
            let edge = &out[idx];
            if edge.background_color.is_none() {
                continue;
            }
            let own = (edge.background_color, edge.background_border);
            let pos = starts[idx];
            let holders: Vec<u32> = self
                .box_extents
                .iter()
                .filter(|b| b.1 <= pos && pos < b.2)
                .map(|b| b.0)
                .collect();
            let cut = if ghost {
                true
            } else if holders.is_empty() {
                nb.is_some_and(|a| self.band_at(a) == Some(own))
            } else {
                !nb.is_some_and(|a| {
                    self.spacer_edges.iter().any(|&(p, id, _, _)| {
                        p <= a && a < p + crate::text::inline::SPACER.len() && holders.contains(&id)
                    })
                })
            };
            if !cut {
                continue;
            }
            // The whole band segment touching that edge: `vendor/gpui`
            // draws a band with the sides of its first run.
            let step: isize = if idx == 0 { 1 } else { -1 };
            let mut i = idx as isize;
            while i >= 0 && (i as usize) < n {
                let r = &out[i as usize];
                if (r.background_color, r.background_border) != own {
                    break;
                }
                cuts.push((i as usize, is_left));
                i += step;
            }
        }
        for (i, is_left) in cuts {
            cut_band_sides(&mut out[i], is_left, !is_left);
        }
    }

    /// Слова строки — куски между пробелами, каждое со счётом пробелов слева.
    /// Visual level runs `(start, end, rtl)` of a line (UAX #9 L1–L2), or none
    /// when the plain path suffices: a left-to-right line without a
    /// right-to-left run, a right-to-left line of one right-to-left run and no
    /// inline box edges (the mirror below places it).
    ///
    /// Box spacers (`inline::SPACER`, U+FEFF) are analysed as neutrals
    /// (U+FFFC, same UTF-8 length): as boundary neutrals X9 would give a
    /// trailing spacer the level of the embedding it follows, while the edges
    /// belong to the parent's level (CSS Writing Modes 4 §2.4).
    pub(crate) fn line_visual_runs(
        &self,
        range: &std::ops::Range<usize>,
    ) -> Vec<(usize, usize, bool)> {
        if range.start >= range.end || range.end > self.text.len() {
            return Vec::new();
        }
        let rtl = self.wrap.rtl;
        let edges = rtl && self.spacer_edges.iter().any(|e| range.contains(&e.0));
        let needs = edges
            || self.text[range.clone()].chars().any(|c| {
                matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
                    || match unicode_bidi::bidi_class(c) {
                        unicode_bidi::BidiClass::R | unicode_bidi::BidiClass::AL => !rtl,
                        unicode_bidi::BidiClass::AN => true,
                        unicode_bidi::BidiClass::L | unicode_bidi::BidiClass::EN => rtl,
                        _ => false,
                    }
            });
        if !needs {
            return Vec::new();
        }
        let mut text: String = self.text.to_string();
        for &at in &self.spacers {
            if text.get(at..at + 3) == Some("\u{feff}") {
                text.replace_range(at..at + 3, "\u{fffc}");
            }
        }
        let forced = if self.plaintext.is_some() && !rtl {
            None
        } else if rtl {
            Some(unicode_bidi::Level::rtl())
        } else {
            Some(unicode_bidi::Level::ltr())
        };
        let info = unicode_bidi::BidiInfo::new(&text, forced);
        let Some(para) = info
            .paragraphs
            .iter()
            .find(|p| p.range.start <= range.start && range.start < p.range.end)
        else {
            return Vec::new();
        };
        if para.level.is_rtl() != rtl {
            return Vec::new();
        }
        let (levels, visual) = info.visual_runs(para, range.clone());
        let runs: Vec<(usize, usize, bool)> = visual
            .into_iter()
            .map(|r| {
                (
                    r.start,
                    r.end,
                    levels.get(r.start).is_some_and(|l| l.is_rtl()),
                )
            })
            .collect();
        let mixed = if rtl {
            runs.iter().any(|r| !r.2)
        } else {
            runs.iter().any(|r| r.2)
        };
        if mixed || edges { runs } else { Vec::new() }
    }

    pub(crate) fn words(&self, range: &std::ops::Range<usize>) -> Vec<Word> {
        let mut out: Vec<Word> = Vec::new();
        let mut start = None;
        let mut spaces = 0usize;
        for (i, ch) in self.text[range.clone()].char_indices() {
            let at = range.start + i;
            // A character-level opportunity (`justify_boundary`) also ends a
            // word: the next unit is placed with one more expansion.
            if let Some(s) = start
                && at > s
                && self.justify_boundary(range.start, at)
            {
                out.push(Word {
                    range: s..at,
                    spaces_before: spaces,
                });
                spaces += 1;
                start = Some(at);
            }
            // Разделитель слов для выключки — не любой пробел. По css-text-3
            // это пробел, неразрывный и идеографический; ТАБУЛЯЦИЯ в него не
            // входит: она доводит строку до своей позиции, и растягивать её
            // нечем. Пока табуляция раскрывалась в пробелы и каждый считался
            // точкой раздачи, остаток размазывался по ней вместо слов.
            if word_separator(ch) {
                if let Some(s) = start.take() {
                    out.push(Word {
                        range: s..at,
                        spaces_before: spaces,
                    });
                }
                spaces += usize::from(!self.ruby_justify);
            } else if ch == '\u{9}' {
                // Табуляция — ГРАНИЦА слова, хотя точкой раздачи и не служит.
                // Её продвижение задаёт позиция табуляции (`Seg::offset`), и
                // внутри слова оно пропадало: строка без пробелов уходила в
                // набор одним куском, и табуляция рисовалась глифом шрифта
                // (`text-indent-tab-positions-001`: `a⇥b⇥c` выходило `abc`).
                if let Some(s) = start.take() {
                    out.push(Word {
                        range: s..at,
                        spaces_before: spaces,
                    });
                }
            } else if start.is_none() {
                start = Some(at);
            }
        }
        if let Some(s) = start {
            out.push(Word {
                range: s..range.end,
                spaces_before: spaces,
            });
        }
        out
    }
}

/// Слово строки и сколько пробелов стоит перед ним от начала строки.
pub(crate) struct Word {
    pub(crate) range: std::ops::Range<usize>,
    pub(crate) spaces_before: usize,
}

/// Точка возможного разрыва.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stop {
    pub(crate) at: usize,
    pub(crate) mandatory: bool,
}

/// Какой край отрезка спрашивают: у конца строки индекс сразу за переводом
/// строки принадлежит прошлому куску, у начала — новому.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Edge {
    Start,
    End,
}

/// Кусок текста между обязательными разрывами вместе со своим набором.
#[derive(Clone)]
pub(crate) struct Seg {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) layout: std::sync::Arc<gpui::LineLayout>,
    /// Сдвиг начала куска внутри своей строки. Нужен табуляции: она рвёт
    /// набор на куски, и каждый следующий начинается со своей позиции табуляции.
    pub(crate) offset: Pixels,
}

/// Длина куска без хвостовых пробелов — они висят за краем строки.
/// Сколько байт схлопываемых пробелов в НАЧАЛЕ строки: по CSS они удаляются
/// вместе с переносом, иначе следующая строка начинается с отступа в пробел.
pub(super) fn skip_leading(chunk: &str) -> usize {
    chunk.len() - chunk.trim_start_matches([' ', '\t']).len()
}

/// Разделитель, который висит за краем строки. Неразрывный пробел сюда НЕ
/// входит: он держит слова вместе и место занимает всегда. Идеографический
/// U+3000 и прочие Zs-разделители ВИСЯТ (`trailing-ideographic-space-002`,
/// `trailing-other-space-separators-001..004`) — сужение набора до
/// 0x20/09/0A теряло 9 зелёных пар, а целевые break-spaces тесты не чинило.
pub(super) fn hangs(ch: char) -> bool {
    matches!(
        ch as u32,
        0x20 | 0x09 | 0x0A | 0x1680 | 0x2000..=0x200A | 0x202F | 0x205F | 0x3000
    )
}

/// Разделитель слов по css-text-3 §8.2: к нему прибавляется `word-spacing`,
/// и по нему же выключка раздаёт остаток строки.
///
/// Обычным пробелом набор не исчерпывается: пока неразрывный в него не
/// входил, `word-spacing` на строке из `&nbsp;` не действовал вовсе
/// (`word-spacing-001`).
/// CJK ideographs, kana and CJK symbols: `text-justify: auto` expands
/// around them (Blink `Character::IsCJKIdeographOrSymbol`).
pub(super) fn justify_ideograph(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x2FDF | 0x3001..=0x303F | 0x3040..=0x30FF | 0x31C0..=0x31FF
        | 0x3200..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0x20000..=0x3FFFF)
}

/// Cursive (joining) scripts: no inter-character expansion inside them
/// (css-text-3 §7.3 `inter-character`, §7.3.1 cursive scripts).
pub(super) fn cursive_script(c: char) -> bool {
    matches!(c as u32,
        0x0600..=0x08FF | 0x1800..=0x18AF
        | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF | 0x10D00..=0x10D3F)
}

pub(super) fn word_separator(ch: char) -> bool {
    // Идеографический пробел U+3000 — ФИКСИРОВАННОЙ ширины и разделителем
    // слов НЕ считается (css-text-3 §word-separator; word-spacing-
    // characters-001): `word-spacing` его не трогает.
    matches!(
        ch as u32,
        0x20 | 0xA0 | 0x1361 | 0x10100 | 0x10101 | 0x1039F | 0x1091F
    )
}

/// Знак управления нулевой ширины: сам не висит, но обрезка хвоста смотрит
/// сквозь него — иначе пробел перед ним перестаёт висеть.
pub(super) fn zero_width(ch: char) -> bool {
    matches!(ch as u32, 0x200B | 0x2060 | 0xFEFF | 0x202A..=0x202E | 0x2066..=0x2069)
}

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО: обрезать ПОДЛОЖКУ прогонов на висящем хвосте
/// строки (css-text §8.1: разделители у конца строки в строку не входят,
/// значит и фон строчной коробки за ними тянуться не должен). Набор при этом
/// рисовался целиком, менялись только `background_color` хвостовых прогонов.
/// Срез css-text + white-space + linebox (1730 пар, 1582 зелёных):
/// * хвост по всему `hangs` — 1573 (−9): три целевых
///   `trailing-other-space-separators-001/003/004` 0.54 → 0.00, но вся семья
///   `trailing-ideographic-space-002/017..025` уходит в «красное видно»:
///   идеографический пробел U+3000 фиксированной ширины и НЕ висит;
/// * без U+3000 — 1580 (−2): целевые три возвращаются в красное (их хвост
///   КОНЧАЕТСЯ на U+3000, обрезка об него спотыкается), ломаются
///   `line-break-anywhere-and-white-space-006/007` (`pre-wrap`) и
///   `word-spacing-characters-002`;
/// * без U+3000 и с пропуском строк, где пробелы СОХРАНЯЮТСЯ (`pre`,
///   `pre-wrap`), — 1582, ровно baseline: +`line-break-anywhere-and-white-
///   space-004`, −`word-spacing-characters-002`.
///
/// То есть висение U+3000 требуют одни пары и запрещают другие: развилка не
/// в знаке, а в том, чем кончается строка. Возвращать вместе с настоящим
/// правилом Phase II (обрезка хвоста в САМОМ разборе строки, а не в подложке).
pub(super) fn trim_hanging(chunk: &str) -> usize {
    // Идеографический пробел тоже не тянет за собой перенос: место он
    // занимает и рисуется, но строку из-за него не рвут — иначе он один
    // уезжал бы на следующую строку.
    //
    // Соединитель слов (U+FEFF) из обрезки ИСКЛЮЧЁН: им помечены распорки
    // строчных коробок, и в них лежит поле — обрезав хвостовую, коробка
    // теряла своё правое поле целиком (`word-space-transform-010`).
    chunk
        .trim_end_matches(|c| c != '\u{feff}' && (hangs(c) || zero_width(c)))
        .len()
}
