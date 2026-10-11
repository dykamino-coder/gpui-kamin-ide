//! Line for paint; split out to keep the owning module within 250 lines.

use crate::text::paragraph::*;
use gpui::{App, Pixels, Point, TextRun, Window, point, px};

impl Paragraph {
    /// Набрать кусок строки и поставить его прогоны в ВИДИМОМ порядке.
    ///
    /// Двунаправленный текст набирается в логическом порядке, а на экран идёт
    /// в видимом. Переставлять ЗНАКИ нельзя — рвётся арабская вязь, поэтому
    /// строка режется на прогоны по уровням встроенности: порядок прогонов
    /// считаем сами, а каждый прогон набирает сам набор. Правому прогону
    /// сторона сообщается знаком управления: внутри него набор и переставит
    /// знаки, и развернёт парные скобки.
    pub(crate) fn paint_line(
        &self,
        range: &std::ops::Range<usize>,
        runs: &[TextRun],
        at: Point<Pixels>,
        suffix: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> (Option<Pixels>, bool) {
        // Пустой отрезок разбору двунаправленности отдавать нельзя: он берёт
        // уровень по первому знаку и падает на конце текста. Пустая строка
        // бывает у абзаца из одних пробелов и после жёсткого разрыва в конце.
        if range.start >= range.end || range.end > self.text.len() {
            return self.paint_empty_line(range, at, suffix, window, cx);
        }
        let base = if self.wrap.rtl {
            unicode_bidi::Level::rtl()
        } else {
            unicode_bidi::Level::ltr()
        };
        // `unicode-bidi: plaintext`: базу КАЖДОГО абзаца (между жёсткими
        // разрывами) выбирает первый сильный знак (UAX9 P2/P3) — разбор без
        // навязанного уровня делает ровно это. Выключка уже решается так же
        // построчно (см. own_align выше).
        let forced = if self.plaintext.is_some() {
            None
        } else {
            Some(base)
        };
        let info = unicode_bidi::BidiInfo::new(&self.text, forced);
        let Some(para) = info
            .paragraphs
            .iter()
            .find(|p| p.range.start <= range.start && range.start < p.range.end)
            .or_else(|| info.paragraphs.first())
        else {
            return (None, false);
        };
        let (levels, visual) = info.visual_runs(para, range.clone());
        let mut x = at.x;
        // Базовая линия набранного текста строки (от верха строки): знак
        // обрыва, набранный ОТДЕЛЬНО, садится на неё (css-overflow-4 §5:
        // маркер — строчный ребёнок блока на линии строки), а не на свою —
        // у строки-замены кегля блока внутри крупного `<span>` своя базовая
        // линия выше на разницу подъёмов (`text-overflow-string-*`).
        // CSS 2.1 §10.8.1: splitting a line into bidi runs must not give
        // smaller-font runs independent baselines. The complete line supplies
        // the ascent/descent used to align every shaped visual piece.
        // A woven-in mark (clamp ellipsis, hyphen) is in the block's font: it
        // sits on the line's baseline instead of moving the run's glyphs to
        // a baseline of its own (`text-wrap-balance-line-clamp-002`: a 4rem
        // ellipsis with `line-height: 1rem` dropped the clamped line out).
        let common_base = if visual.len() > 1 || !suffix.is_empty() {
            self.base_of(range).map(px)
        } else {
            None
        };
        let mut line_base: Option<Pixels> = common_base;
        let mut line_exact = false;
        let pieces = visual.clone();
        for run in visual.into_iter() {
            let rtl = levels.get(run.start).is_some_and(|l| l.is_rtl());
            // Знак обрыва — у обрезанного КРАЯ: обычно это логический
            // конец строки; при письме справа налево контейнер режет левый
            // край, то есть логическое НАЧАЛО — знак идёт префиксом
            // первого прогона.
            // Строка-замена (`text-overflow: "…"`) рисуется ОТДЕЛЬНЫМ
            // набором — как и при письме справа налево, и как это уже
            // делает `paint_justified`. Вплетение её в набор последнего
            // прогона (`shape` приклеивает суффикс к последнему куску)
            // отдавало ей шрифт И кегль обрезанного куска: эмодзи-маркер в
            // Ahem не рисовался вовсе (`text-overflow-string-003`), а
            // «你好 🟢» выходил кеглем 30px (`-013`). Многоточие и знак
            // переноса остаются вплетёнными: они обязаны сесть на базовую
            // линию строки (`hyphens-manual-011`).
            let own_mark = !suffix.is_empty() && self.overflow_marker.as_deref() == Some(suffix);
            let (tail, at_start) = if self.wrap.rtl {
                (if run.start == range.start { suffix } else { "" }, true)
            } else if own_mark {
                ("", false)
            } else {
                (if run.end == range.end { suffix } else { "" }, false)
            };
            // Band sides at this piece's visual edges (see `cut_piece_sides`).
            let piece_runs;
            let runs: &[TextRun] = if self.runs.iter().any(|r| r.background_color.is_some()) {
                let vis = self.visual_chars(pieces.iter().map(|r| {
                    (
                        r.start,
                        r.end,
                        levels.get(r.start).is_some_and(|l| l.is_rtl()),
                    )
                }));
                let (left, right) = visual_neighbours(&self.text, &vis, &run);
                let mut mid = slice_runs(runs, &run);
                self.cut_piece_sides(&mut mid, &run, rtl, left, right);
                let total: usize = runs.iter().map(|r| r.len).sum();
                let mut all = slice_runs(runs, &(0..run.start));
                all.extend(mid);
                all.extend(slice_runs(runs, &(run.end..total)));
                piece_runs = all;
                &piece_runs
            } else {
                runs
            };
            let Some(shaped) = self.shape_with_mark(&run, runs, rtl, tail, at_start, window) else {
                continue;
            };
            let width = shaped.width;
            let base = (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent;
            if common_base.is_none() {
                line_base = Some(line_base.map_or(base, |b: Pixels| b.max(base)));
            }
            line_exact |= shaped
                .runs
                .iter()
                .any(|r| window.text_system().pixel_exact_glyphs(r.font_id));
            if at_start && !tail.is_empty() {
                // Знак обрыва СЛЕВА от куска: рисуется на своём месте, а
                // кусок сдвигается на его ширину.
                let ell = self.suffix_width(tail, run.start, window);
                self.paint_suffix(
                    tail,
                    run.start,
                    point(x, at.y),
                    Some(common_base.unwrap_or(base)),
                    line_exact,
                    window,
                    cx,
                );
                x += ell;
            }
            let at = point(at.x, at.y + common_base.map_or(px(0.), |line| line - base));
            // Подложка прогона (`background` на `<span>`) рисуется ОТДЕЛЬНЫМ
            // вызовом: `paint` кладёт только глифы. Пока его не звали, фон
            // строчного элемента не появлялся вовсе — проверено пробой, где
            // `background: green; color: transparent` давал пустую страницу.
            self.paint_run_background(&shaped, point(x, at.y), window, cx);
            let origin = self.text_raster_origin(&shaped, point(x, at.y), window);
            // css-text-decor-3 §2.1 «underlines and overlines … are drawn
            // below the text, line-throughs above it» (painting order).
            let decor = self.decor_on();
            if decor {
                self.paint_decor_shaped(
                    &run,
                    &shaped,
                    point(x, at.y),
                    px(0.),
                    rtl,
                    range,
                    false,
                    window,
                );
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
                self.paint_decor_shaped(
                    &run,
                    &shaped,
                    point(x, at.y),
                    px(0.),
                    rtl,
                    range,
                    true,
                    window,
                );
            }
            if !rtl && !self.emph_spans.is_empty() {
                self.paint_emphasis(&run, &shaped, point(x, at.y), window, cx);
            }
            x += width;
        }
        if self.decor_on() {
            self.flush_decor(window);
        }
        // Строка-замена — за текстом строки, своим шрифтом и кеглем.
        if !self.wrap.rtl && !suffix.is_empty() && self.overflow_marker.as_deref() == Some(suffix) {
            let anchor = range.end.saturating_sub(1).max(range.start);
            self.paint_suffix(
                suffix,
                anchor,
                point(x, at.y),
                line_base,
                line_exact,
                window,
                cx,
            );
        }
        (line_base, line_exact)
    }
}
