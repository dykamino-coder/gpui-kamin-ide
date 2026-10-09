//! Отрисовка строк: выделение, строка, маркеры.

use crate::text::paragraph::*;

impl Paragraph {
    /// Прогоны с подложкой на выделенном куске: прогон нельзя раскрасить
    /// наполовину, поэтому попавшие на границу режутся надвое.
    pub(crate) fn runs_with_selection(&self, from: usize, to: usize) -> Vec<TextRun> {
        if from >= to {
            return self.runs.clone();
        }
        let mut out = Vec::with_capacity(self.runs.len() + 2);
        let mut at = 0usize;
        for run in &self.runs {
            let end = at + run.len;
            let mut cut = |start: usize, stop: usize, selected: bool| {
                if stop <= start {
                    return;
                }
                let mut piece = run.clone();
                piece.len = stop - start;
                piece.background_color = selected.then_some(self.highlight);
                out.push(piece);
            };
            cut(at, end.min(from), false);
            cut(at.max(from), end.min(to), true);
            cut(at.max(to), end, false);
            at = end;
        }
        out
    }

    /// Тянуть выделение мышью.
    pub(crate) fn track_selection(
        &self,
        global: &GlobalElementId,
        segs: &[Seg],
        bounds: Bounds<Pixels>,
        hitbox: Hitbox,
        window: &mut Window,
    ) {
        let inside = hitbox.is_hovered(window);
        if inside {
            window.set_cursor_style(gpui::CursorStyle::IBeam, &hitbox);
        }
        // Замыкания живут дольше кадра, поэтому берут СВОЙ снимок раскладки.
        let probe = Paragraph {
            plaintext: self.plaintext,
            lines_reversed: self.lines_reversed,
            flow: self.flow.clone(),
            text: self.text.clone(),
            spans: self.spans.clone(),
            word_spans: self.word_spans.clone(),
            letter_spans: self.letter_spans.clone(),
            shift_spans: self.shift_spans.clone(),
            lh_spans: self.lh_spans.clone(),
            rel_spans: self.rel_spans.clone(),
            atoms: Vec::new(),
            atom_boxes: self.atom_boxes.clone(),
            atom_fit: Default::default(),
            strut: self.strut,
            run_metrics: self.run_metrics.clone(),
            edge_spans: self.edge_spans.clone(),
            ruby_trim: self.ruby_trim,
            emph_spans: self.emph_spans.clone(),
            decor_spans: Vec::new(),
            decor_pending: Default::default(),
            box_spans: self.box_spans.clone(),
            strut_run: None,
            strut_box: self.strut_box,
            ortho_limit: self.ortho_limit,
            runs: Vec::new(),
            font_size: self.font_size,
            line_height: self.line_height,
            align: self.align,
            align_last: self.align_last,
            ruby_justify: self.ruby_justify,
            justify_chars: self.justify_chars,
            ruby_unit: self.ruby_unit,
            ruby_base_sink: None,
            letter_spacing: self.letter_spacing,
            word_spacing: self.word_spacing,
            vertical: self.vertical,
            vertical_rl: self.vertical_rl,
            vertical_central_baseline: self.vertical_central_baseline,
            rotated_central: self.rotated_central,
            vertical_ccw: self.vertical_ccw,
            selection_vertical: self.selection_vertical,
            vertical_layout_origin: self.vertical_layout_origin,
            glyph_nudge: self.glyph_nudge,
            opaque_text_origin: self.opaque_text_origin,
            width_nudge: self.width_nudge,
            indent_basis: self.indent_basis,
            vertical_inline: self.vertical_inline,
            hanging: self.hanging,
            indent: self.indent,
            spacers: self.spacers.clone(),
            spacer_edges: self.spacer_edges.clone(),
            box_extents: self.box_extents.clone(),
            id: None,
            highlight: self.highlight,
            wrap: self.wrap,
            lines: self.lines.clone(),
            clamp: self.clamp,
            clamp_force: self.clamp_force,
            fit_spacing_scalable: self.fit_spacing_scalable,
            fit_line_height_fixed: self.fit_line_height_fixed,
            text_overflow: false,
            overflow_marker: None,
            clamp_marker: None,
            clamp_tag: None,
            unbalanced_steps: None,
            marker_font: None,
            marker_size: None,
            marker_color: None,
            fit: self.fit,
            tab_stop: self.tab_stop.clone(),
            hyphen: self.hyphen.clone(),
            hyphen_w: std::cell::RefCell::new(self.hyphen_w.borrow().clone()),
            overlays: Vec::new(),
        };
        let segs = segs.to_vec();
        window.with_element_state::<Selection, _>(global, |state, window| {
            let st = std::rc::Rc::new(std::cell::Cell::new(state.unwrap_or_default()));
            let index_at = {
                let probe = std::rc::Rc::new(probe);
                let segs = std::rc::Rc::new(segs);
                move |p: Point<Pixels>| probe.index_at(&segs, bounds, p)
            };

            let down = st.clone();
            let at_down = index_at.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, _cx| {
                if !phase.bubble() || e.button != MouseButton::Left || !inside {
                    return;
                }
                let i = at_down(e.position);
                down.set(Selection {
                    anchor: i,
                    head: i,
                    dragging: true,
                });
                window.refresh();
            });

            let mv = st.clone();
            let at_move = index_at.clone();
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _cx| {
                if !phase.bubble() {
                    return;
                }
                let mut s = mv.get();
                if !s.dragging {
                    return;
                }
                let i = at_move(e.position);
                if s.head != i {
                    s.head = i;
                    mv.set(s);
                    window.refresh();
                }
            });

            let up = st.clone();
            window.on_mouse_event(move |_e: &MouseUpEvent, phase, _window, _cx| {
                if !phase.bubble() {
                    return;
                }
                let mut s = up.get();
                if s.dragging {
                    s.dragging = false;
                    up.set(s);
                }
            });
            ((), st.get())
        });
    }

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
            // Пустой отрезок с ХВОСТОМ — это строка обрыва `line-clamp`, у
            // которой под многоточие не осталось места ни для одного слова
            // (`clamp_lines` схлопывает диапазон в `head..head`). Знак обрыва
            // рисуется в цикле по прогонам ниже, поэтому ранний выход уносил
            // и его: коробка занимала высоту, но многоточия не показывала
            // (`text-wrap-balance-line-clamp-004`).
            if !suffix.is_empty() && !self.text.is_empty() {
                let anchor = range.start.min(self.text.len().saturating_sub(1));
                self.paint_suffix(suffix, anchor, at, None, false, window, cx);
            }
            return (None, false);
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
        let common_base = if visual.len() > 1 {
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
                    (r.start, r.end, levels.get(r.start).is_some_and(|l| l.is_rtl()))
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
                self.paint_decor_shaped(&run, &shaped, point(x, at.y), px(0.), rtl, range, false, window);
            }
            let _ = shaped.paint(origin, self.line_height, gpui::TextAlign::Left, None, window, cx);
            if decor {
                self.paint_decor_shaped(&run, &shaped, point(x, at.y), px(0.), rtl, range, true, window);
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
            self.paint_suffix(suffix, anchor, point(x, at.y), line_base, line_exact, window, cx);
        }
        (line_base, line_exact)
    }

    /// Строковый маркер несёт шрифт контейнера; многоточие и знак переноса
    /// остаются в шрифте прогона у среза.
    pub(crate) fn style_marker_run(&self, mark: &str, run: &mut TextRun) {
        // Знак обрыва — содержимое САМОГО БЛОКА, а не куска, на котором
        // строка оборвалась: кегль у него блочный (css-overflow-3 §4.1,
        // «the ellipsis is styled as the block»). Прогон брался с места
        // обрыва вместе со своим кеглем, и внутри `<span style="font-size:
        // 1rem">` в блоке с `4rem` многоточие выходило вчетверо уже нужного
        // (`text-wrap-balance-line-clamp-002`: место под него при подборе
        // колонки считалось 8.8 точки вместо 35.2).
        run.font_size = None;
        // Знак обрыва — анонимный строчный ребёнок САМОГО БЛОКА
        // (css-overflow-4 §5.3 block-ellipsis: «wrapped in an anonymous
        // inline whose parent is the block container»): шрифт, кегль и цвет —
        // блочные, рамки и фона куска у среза у него нет. Эталоны:
        // `block-ellipsis-005` (знак за `<span>` 1.5em bold italic — обычный
        // teal блока), `webkit-line-clamp-031` (за жирным — нежирный).
        if self.is_block_mark(mark)
            && let Some(f) = self.marker_font.as_ref()
        {
            run.font = f.clone();
            run.font_size = self.marker_size;
            if let Some(c) = self.marker_color {
                run.color = c;
            }
            run.background_color = None;
            run.background_border = None;
            run.background_pad = Default::default();
            run.background_radius = px(0.);
            return;
        }
        if let (Some(m), Some(f)) = (self.overflow_marker.as_deref(), self.marker_font.as_ref())
            && mark == m
        {
            run.font = f.clone();
            // Кегль СТРОКИ-ЗАМЕНЫ — блочный, а не базовый кегль абзаца:
            // базовый равен `biggest`, и внутри `<span>` крупнее блока
            // маркер выходил втрое шире (`text-overflow-string-*`: под
            // строку резервировалось ~90 точек вместо 20).
            run.font_size = self.marker_size;
        }
    }

    /// Набор с знаком обрыва в начале или в конце куска.
    pub(crate) fn shape_with_mark(
        &self,
        range: &std::ops::Range<usize>,
        runs: &[TextRun],
        rtl: bool,
        suffix: &str,
        at_start: bool,
        window: &mut Window,
    ) -> Option<gpui::ShapedLine> {
        if !at_start || suffix.is_empty() {
            return self.shape(range, runs, rtl, suffix, window);
        }
        // Префикс: знак дорисовывается отдельным вызовом слева, а сам кусок
        // набирается без него (вплетение в шейп меняло бы кернинг начала).
        self.shape(range, runs, rtl, "", window)
    }
}

/// Память выделения между кадрами: границы в байтах текста абзаца.
#[derive(Default, Clone, Copy)]
pub(crate) struct Selection {
    pub(crate) anchor: usize,
    pub(crate) head: usize,
    pub(crate) dragging: bool,
}

impl Selection {
    pub(crate) fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }
}
