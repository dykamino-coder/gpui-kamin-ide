//! Замеры абзаца: поля строк, вырезы обтекания, отступы, висячие знаки, ширины, min-content.

use crate::text::paragraph::*;
use gpui::{Pixels, Window, px};

impl Paragraph {
    /// Вырез строки номер `line_no`: (слева, справа).
    /// Надбавки строки сверху и снизу от сдвинутых по вертикали кусков.
    ///
    /// Сдвиг `vertical-align` не просто двигает знаки — он РАСТИТ строчную
    /// коробку (CSS 2.1 §10.8): её верх и низ берутся по объединению всех
    /// кусков после выравнивания. На каждую строку своя пара: абзац с
    /// надстрочным знаком в одной строке не должен раздувать остальные.
    pub(crate) fn line_padding(&self) -> Vec<(f32, f32)> {
        if self.shift_spans.is_empty()
            && self.lh_spans.is_empty()
            && self.atom_boxes.is_empty()
            && self.edge_spans.is_empty()
            && self.box_spans.is_empty()
            && self.emph_spans.is_empty()
        {
            return vec![(0.0, 0.0); self.lines.len()];
        }
        let lh = f32::from(self.line_height);
        let last_line = self.lines.len().saturating_sub(1);
        self.lines
            .iter()
            .enumerate()
            .map(|(line_no, line)| {
                let (mut above, mut below) = (0.0f32, 0.0f32);
                let boxes = !self.box_spans.is_empty() && self.run_metrics.len() == self.runs.len();
                if boxes {
                    let (top, bot) = self.line_extents(&line.range);
                    let a = self.line_base(&line.range);
                    above = top - a;
                    below = bot - (lh - a);
                }
                // Кусок со своей `line-height` растит строку симметрично:
                // полулидинг его коробки отступа ложится сверху и снизу
                // (§10.8). Блочное значение уже учтено высотой строки.
                for (range, lh) in self.lh_spans.iter().filter(|_| !boxes) {
                    if range.end <= line.range.start || range.start >= line.range.end {
                        continue;
                    }
                    let half = (f32::from(*lh) - f32::from(self.line_height)) / 2.0;
                    if half > 0.0 {
                        above = above.max(half);
                        below = below.max(half);
                    }
                }
                for (range, dy) in self.shift_spans.iter().filter(|_| !boxes) {
                    if range.end <= line.range.start || range.start >= line.range.end {
                        continue;
                    }
                    // Ось сдвига смотрит вниз: отрицательное поднимает знак
                    // над строкой, положительное опускает.
                    let v = f32::from(*dy);
                    above = above.max(-v);
                    below = below.max(v);
                }
                // Атом растит строку на то, чем его коробка полей выходит за
                // струт (§10.8: строчная коробка — от верха самой высокой
                // коробки до низа самой низкой). `top`/`bottom` решаются
                // ПОСЛЕ остальных: они равняются по уже собранной строке и
                // растят её, только если сами выше (§10.8.1).
                let inside = |at: usize| at >= line.range.start && at < line.range.end;
                let a = self.line_base(&line.range);
                for b in self.atom_boxes.iter().filter(|b| inside(b.at)) {
                    if matches!(b.align, AtomAlign::Top | AtomAlign::Bottom) {
                        continue;
                    }
                    let t = self.atom_top(b);
                    // Аннотация руби растит строку, только выходя за неё:
                    // полулидинг строки она занимает даром (css-ruby-1 §3.4).
                    let over = if line_no == 0 && self.ruby_trim.0 {
                        0.0
                    } else {
                        b.over
                    };
                    let under = if line_no == last_line && self.ruby_trim.1 {
                        0.0
                    } else {
                        b.under
                    };
                    above = above.max(-(t - over) - a);
                    below = below.max(t + b.h + under - (lh - a));
                }
                // Знак акцента стоит над (под) коробкой содержимого своего
                // прогона и растит строку, только выходя за неё.
                for EmphSpan {
                    range,
                    under,
                    size: h,
                    ..
                } in &self.emph_spans
                {
                    if range.end <= line.range.start || range.start >= line.range.end {
                        continue;
                    }
                    let mut at = 0usize;
                    let metrics = self
                        .runs
                        .iter()
                        .zip(&self.run_metrics)
                        .find_map(|(run, m)| {
                            let s = at;
                            at += run.len;
                            (range.start >= s && range.start < at).then_some(*m)
                        });
                    let Some((ra, rd)) = metrics else { continue };
                    // Срез текстовой коробки знак не растит так же, как
                    // аннотацию (`text-box-trim-ruby-start-002`).
                    if (*under && line_no == last_line && self.ruby_trim.1)
                        || (!*under && line_no == 0 && self.ruby_trim.0)
                    {
                        continue;
                    }
                    if *under {
                        below = below.max(rd + h - (lh - a));
                    } else {
                        above = above.max(ra + h - a);
                    }
                }
                // Прижатые к краю — атомы и куски текста — после всех
                // остальных: строка растёт, только если такой кусок выше.
                let edges = self
                    .atom_boxes
                    .iter()
                    .filter(|b| inside(b.at))
                    .filter_map(|b| match b.align {
                        AtomAlign::Top => Some((true, b.h)),
                        AtomAlign::Bottom => Some((false, b.h)),
                        _ => None,
                    })
                    .chain(
                        self.edge_spans
                            .iter()
                            .filter(|(r, _, _)| {
                                r.start < line.range.end && r.end > line.range.start
                            })
                            .map(|(_, top, h)| (*top, *h)),
                    );
                for (top, h) in edges {
                    let total = lh + above + below;
                    if h <= total {
                        continue;
                    }
                    if top {
                        below += h - total;
                    } else {
                        above += h - total;
                    }
                }
                (above, below)
            })
            .collect()
    }

    pub(crate) fn flow_cut(&self, line_no: usize) -> (f32, f32) {
        if self.flow.0.is_empty() && self.flow.1.is_empty() {
            return (0.0, 0.0);
        }
        let lh = f32::from(self.line_height);
        let (y0, y1) = (line_no as f32 * lh, (line_no as f32 + 1.0) * lh);
        let l = self
            .flow
            .0
            .iter()
            .map(|f| f.cut(y0, y1))
            .fold(0.0f32, f32::max);
        let r = self
            .flow
            .1
            .iter()
            .map(|f| f.cut(y0, y1))
            .fold(0.0f32, f32::max);
        (l, r)
    }

    /// Отступ ЭТОЙ строки в точках.
    ///
    /// Доля считается от ширины строки (css-text-3 §7.1: процент берётся от
    /// ширины содержащего блока), поэтому предел приходит сюда: при замере по
    /// содержимому его нет, и доля обращается в ноль — как в браузере.
    pub(crate) fn indent_of(
        &self,
        head_of_part: bool,
        first_part: bool,
        limit: Option<Pixels>,
    ) -> Pixels {
        let own = if self.indent.each_line {
            head_of_part
        } else {
            head_of_part && first_part
        };
        if own == self.indent.hanging {
            return px(0.);
        }
        let basis = limit.map(|l| self.indent_basis.unwrap_or(l));
        let pct = self.indent.pct * f32::from(basis.unwrap_or(px(0.)));
        px(self.indent.px + pct)
    }

    /// Сколько байт в начале строки свисает за левый край.
    ///
    /// Свисает только открывающий знак и только в начале ПЕРВОЙ строки
    /// абзаца: место он занимает в поле, а не в колонке, поэтому в ширину
    /// строки не входит.
    pub(crate) fn hang_first(&self, start: usize) -> usize {
        if !self.hanging.first || start != 0 {
            return 0;
        }
        match self.text[start..].chars().next() {
            Some(ch) if is_opening(ch) => ch.len_utf8(),
            _ => 0,
        }
    }

    /// Сколько байт в конце строки свисает за правый край.
    pub(crate) fn hang_last(&self, end: usize, closing_line: bool, over: bool) -> usize {
        let Some(ch) = self.text[..end].chars().next_back() else {
            return 0;
        };
        let hangs = (self.hanging.last && closing_line && is_closing(ch))
            || (self.hanging.force_end && is_stop(ch))
            // `allow-end` свисает ТОЛЬКО когда строка иначе не влезает —
            // в отличие от `force-end`, который свисает всегда. Пока разницы
            // не было, строки рвались на знак позже, чем надо.
            || (self.hanging.allow_end && over && is_stop(ch));
        if hangs { ch.len_utf8() } else { 0 }
    }

    /// Куски между обязательными разрывами: набор не принимает перевод строки,
    /// поэтому мерить приходится по кускам, а положения знаков сшивать.
    ///
    /// Результат запоминается: раскладка спрашивает размер абзаца по многу раз
    /// за кадр (перебор ширин в гибком контейнере), а набор строки — самая
    /// дорогая операция здесь.
    pub(crate) fn measure(&self, window: &mut Window) -> Vec<Seg> {
        let key = self.measure_key();
        if let Some(hit) = MEASURED.with(|c| {
            c.borrow()
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
        }) {
            return hit;
        }
        let out = self.measure_uncached(window);
        MEASURED.with(|c| {
            let mut cache = c.borrow_mut();
            if cache.len() >= MEASURE_CACHE {
                cache.remove(0);
            }
            cache.push((key, out.clone()));
        });
        out
    }

    /// Ключ памяти замера: от чего зависит положение знаков.
    pub(crate) fn measure_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.text.hash(&mut h);
        f32::from(self.font_size).to_bits().hash(&mut h);
        f32::from(self.letter_spacing).to_bits().hash(&mut h);
        f32::from(self.word_spacing).to_bits().hash(&mut h);
        self.tab_stop.hash_into(&mut h);
        for run in &self.runs {
            run.len.hash(&mut h);
            run.font_size.map(|s| f32::from(s).to_bits()).hash(&mut h);
            run.font.family.hash(&mut h);
            run.font.weight.0.to_bits().hash(&mut h);
            (run.font.style as u8).hash(&mut h);
            // Возможности OpenType меняют и подстановку, и продвижение
            // (`vert`, `hwid`) — без них кэш отдавал чужой набор.
            for (tag, value) in run.font.features.tag_value_list() {
                tag.hash(&mut h);
                value.hash(&mut h);
            }
        }
        h.finish()
    }

    /// Ключ РАЗРЕЗА: замер плюс всё, от чего зависит перенос и ширины строк.
    pub(crate) fn split_key(&self, limit: Option<Pixels>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.measure_key().hash(&mut h);
        limit.map(|l| f32::from(l).to_bits()).hash(&mut h);
        self.indent_basis
            .map(|l| f32::from(l).to_bits())
            .hash(&mut h);
        let w = &self.wrap;
        [
            w.nowrap,
            w.break_spaces,
            w.break_all,
            w.anywhere,
            w.keep_all,
            w.break_word,
            w.wrap_anywhere,
            w.rtl,
            w.balance,
            w.keep_spaces,
        ]
        .hash(&mut h);
        // Per-piece wrap rules (`white-space`/`word-break` of a nested inline
        // or of a `display: contents` element) change the breaks too: without
        // them a `nowrap` run reused the cached lines of an identical text
        // laid out under `normal` (`white-space-applies-to-text-001`).
        format!("{:?}{:?}", self.wrap, self.spans).hash(&mut h);
        self.indent.px.to_bits().hash(&mut h);
        for f in self.flow.0.iter().chain(self.flow.1.iter()) {
            f.hash_bits().hash(&mut h);
        }
        self.indent.pct.to_bits().hash(&mut h);
        self.indent.each_line.hash(&mut h);
        self.indent.hanging.hash(&mut h);
        let hg = &self.hanging;
        [hg.first, hg.last, hg.force_end, hg.allow_end].hash(&mut h);
        self.clamp.hash(&mut h);
        self.clamp_force.hash(&mut h);
        self.text_overflow.hash(&mut h);
        self.overflow_marker.hash(&mut h);
        self.clamp_marker.hash(&mut h);
        self.marker_size
            .map(|s| f32::from(s).to_bits())
            .hash(&mut h);
        self.hyphen.hash(&mut h);
        self.spacers.hash(&mut h);
        for (r, v) in self.word_spans.iter().chain(&self.letter_spans) {
            r.start.hash(&mut h);
            r.end.hash(&mut h);
            f32::from(*v).to_bits().hash(&mut h);
        }
        h.finish()
    }

    pub(crate) fn measure_uncached(&self, window: &mut Window) -> Vec<Seg> {
        let mut out = Vec::new();
        let mut start = 0usize;
        let mut offset = px(0.);
        loop {
            // Кусок кончается переводом строки ИЛИ табуляцией: табуляция — не
            // знак со своей шириной, а прыжок к следующей позиции табуляции, и
            // отдавать её набору нечего (`break-spaces-tab`).
            let end = self.text[start..]
                .find(['\n', '\t', SOFT_HYPHEN])
                .map(|i| start + i)
                .unwrap_or(self.text.len());
            let runs = slice_runs(&self.runs, &(start..end));
            let layout = window
                .text_system()
                .with_ligature_breaking(false)
                .layout_line_spaced(
                    &self.text[start..end],
                    self.font_size,
                    &runs,
                    None,
                    self.letter_spacing,
                );
            // Ширина куска вместе с трекингом кусков и `word-spacing`: набор
            // их не знает, `x_at` добавляет их сам — и позиция табуляции за
            // куском обязана их учесть (`word-spacing-characters-001`:
            // табуляция после растянутых пробелов вставала раньше).
            let width = layout.width + self.seg_extra(start, end);
            out.push(Seg {
                start,
                end,
                layout,
                offset,
            });
            if end >= self.text.len() {
                break;
            }
            let mark = self.text[end..].chars().next().unwrap_or('\n');
            match mark {
                '\n' => offset = px(0.),
                // Мягкий перенос своей ширины не имеет: он лишь ПОЗВОЛЯЕТ
                // разрыв. Пока он доезжал до набора, шрифт давал ему ширину
                // дефиса, и слово рвалось раньше времени (`hyphens-manual-011`:
                // «Deoxy-ribo-» вместо «Deoxyribo-»).
                SOFT_HYPHEN => offset += width,
                _ => {
                    let x = f32::from(offset + width);
                    let next = self.tab_stop.next(end, x);
                    offset = px(next);
                }
            }
            start = end + mark.len_utf8();
        }
        out
    }

    /// Ширина отрезка строки.
    ///
    /// Положения знаков считаются от начала своего куска, поэтому границу
    /// надо толковать по её роли: КОНЕЦ отрезка сразу за переводом строки —
    /// это конец прошлого куска, а НАЧАЛО с тем же индексом — начало нового.
    /// Иначе строка, открывающая новый кусок, получала ширину со знаком минус
    /// и уезжала за край коробки.
    pub(crate) fn span(&self, segs: &[Seg], from: usize, to: usize) -> Pixels {
        if to <= from {
            return px(0.);
        }
        let width = self.x_at(segs, to, Edge::End) - self.x_at(segs, from, Edge::Start);
        if width < px(0.) { px(0.) } else { width }
    }

    /// Положение знака от начала своего куска.
    /// Добавка к ширине набранного куска `start..end`: трекинг кусков сверх
    /// общего и `word-spacing` у пробелов (то же, что `x_at` прибавляет к
    /// положению знака в конце куска).
    pub(crate) fn seg_extra(&self, start: usize, end: usize) -> Pixels {
        let mut extra = px(0.);
        if self.letter_spans.is_empty() && self.word_spacing == px(0.) && self.word_spans.is_empty()
        {
            return extra;
        }
        for (off, ch) in self.text[start..end].char_indices() {
            let at = start + off;
            if let Some((_, v)) = self.letter_spans.iter().find(|(r, _)| r.contains(&at)) {
                extra += *v - self.letter_spacing;
            }
            if word_separator(ch) {
                extra += self
                    .word_spans
                    .iter()
                    .find(|(r, _)| r.contains(&at))
                    .map(|(_, v)| *v)
                    .unwrap_or(self.word_spacing);
            }
        }
        extra
    }

    pub(crate) fn x_at(&self, segs: &[Seg], i: usize, edge: Edge) -> Pixels {
        let i = i.min(self.text.len());
        let after_break = edge == Edge::End && i > 0 && self.text.as_bytes()[i - 1] == b'\n';
        let seg = if after_break {
            segs.iter().find(|s| s.end + 1 == i)
        } else {
            segs.iter().find(|s| i <= s.end)
        };
        let Some(seg) = seg.or_else(|| segs.last()) else {
            return px(0.);
        };
        let base = if i >= seg.end {
            seg.layout.width
        } else if i <= seg.start {
            px(0.)
        } else {
            seg.layout.x_for_index(i - seg.start)
        };
        // Сдвиг куска внутри строки: его задаёт табуляция перед ним.
        let mut base = base + seg.offset;
        if !self.letter_spans.is_empty() {
            let upto = i.min(seg.end);
            for (off, _) in self.text[seg.start..upto].char_indices() {
                let at = seg.start + off;
                if let Some((_, v)) = self.letter_spans.iter().find(|(r, _)| r.contains(&at)) {
                    base += *v - self.letter_spacing;
                }
            }
        }
        if self.word_spacing == px(0.) && self.word_spans.is_empty() {
            return base;
        }
        // Набор про `word-spacing` не знает: знак сдвинут на столько добавок,
        // сколько пробелов осталось позади него внутри куска. Добавка у
        // каждого пробела СВОЯ — заданная на том куске, в который он попал.
        let upto = i.min(seg.end);
        let mut extra = px(0.);
        for (off, _) in self.text[seg.start..upto]
            .char_indices()
            .filter(|(_, c)| word_separator(*c))
        {
            let at = seg.start + off;
            extra += self
                .word_spans
                .iter()
                .find(|(r, _)| r.contains(&at))
                .map(|(_, v)| *v)
                .unwrap_or(self.word_spacing);
        }
        base + extra
    }

    /// Ширина по минимальному содержимому — самый широкий кусок, который
    /// разорвать нельзя.
    ///
    /// Нужна раскладке: под неё она меряет высоту, когда ширина ещё не
    /// решена. Ноль тут не годится — по нулю строка рвётся на каждом знаке, и
    /// коробка выходит во много раз выше настоящей.
    pub(crate) fn min_content(&self, window: &mut Window) -> Pixels {
        self.min_content_with(None, window)
    }

    /// Min-content contribution with `text-indent` (css-text-3 §7.1, CSS 2.1
    /// §16.1): the content is broken at EVERY soft wrap opportunity, and the
    /// indent (percentages count as zero) is added to the first piece of each
    /// indented line — a negative indent makes that piece narrower
    /// (`text-indent-intrinsic-003/004`, «negative-intrinsic-min»).
    pub(crate) fn min_content_indented(&self, window: &mut Window) -> Pixels {
        self.min_content_with(Some(px(self.indent.px)), window)
    }

    pub(crate) fn min_content_with(&self, indent: Option<Pixels>, window: &mut Window) -> Pixels {
        let segs = self.measure(window);
        let mut best = px(0.);
        let mut start = 0usize;
        // Starts a line that carries the indent (`each-line`: also after a
        // forced break).
        let mut indented = indent.is_some();
        let each_line = self.indent.each_line;
        let mut chunk = |from: usize, to: usize, this: &Self, lead: Pixels| {
            let end = if this.spaces_are_content() {
                to
            } else {
                this.hang_tail(from, to)
            };
            // Трекинг ПОСЛЕДНЕГО знака куска на конце строки не действует
            // (css-text-3 §8.2) — `lay_in` его вычитает, а минимум по
            // содержимому считал, и кусок выходил шире на `letter-spacing`.
            let w = this.span(&segs, from, end) - this.tail_spacing(end) + lead;
            let w = if w < px(0.) { px(0.) } else { w };
            if w > best {
                best = w;
            }
        };
        // `overflow-wrap: anywhere` — единственное из семейства, что меняет
        // размер по минимальному содержимому: слово рвётся и здесь, поэтому
        // точками счёта становятся ВСЕ границы знаков (css-text-3 §5.5).
        let stops: Vec<Stop> = if self.wrap.wrap_anywhere {
            self.text
                .char_indices()
                .skip(1)
                .map(|(at, _)| Stop {
                    at,
                    mandatory: false,
                })
                .collect()
        } else {
            // Куски со своим `overflow-wrap: anywhere` добавляют границы
            // знаков только внутри себя.
            let mut stops = self.opportunities();
            for (range, w) in &self.spans {
                if !w.wrap_anywhere {
                    continue;
                }
                for (i, _) in self.text[range.clone()].char_indices().skip(1) {
                    stops.push(Stop {
                        at: range.start + i,
                        mandatory: false,
                    });
                }
            }
            stops.sort_by_key(|s| (s.at, !s.mandatory));
            stops.dedup_by_key(|s| s.at);
            stops
        };
        let lead = |on: bool| if on { indent.unwrap_or(px(0.)) } else { px(0.) };
        for stop in stops {
            if stop.at <= start {
                continue;
            }
            chunk(start, stop.at, self, lead(indented));
            indented = indent.is_some() && each_line && stop.mandatory;
            start = stop.at;
        }
        chunk(start, self.text.len(), self, lead(indented));
        best
    }
}

/// Забыть замеры. Зовётся при разборе новой страницы: одно и то же имя
/// семейства на разных страницах означает РАЗНЫЕ шрифты (`@font-face`), и
/// старые положения знаков стали бы чужими.
pub fn forget_measures() {
    MEASURED.with(|c| c.borrow_mut().clear());
    SPLITS.with(|c| c.borrow_mut().clear());
}

thread_local! {
    /// Память разрезов: ключ разреза → готовые строки.
    pub(super) static SPLITS: std::cell::RefCell<
        std::collections::HashMap<u64, std::rc::Rc<Vec<Line>>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Сколько замеров абзацев помнить между кадрами.
const MEASURE_CACHE: usize = 64;

/// Память подбора кегля: ключ абзаца → найденный множитель.
pub(super) fn remember_fit(key: u64, k: f32) {
    FITTED.with(|c| {
        let mut cache = c.borrow_mut();
        if let Some(hit) = cache.iter_mut().find(|(hit, _)| *hit == key) {
            hit.1 = k;
            return;
        }
        if cache.len() >= MEASURE_CACHE {
            cache.remove(0);
        }
        cache.push((key, k));
    });
}

thread_local! {
    /// Найденные множители `text-fit`: ключ абзаца → во сколько раз крупнее.
    pub(super) static FITTED: std::cell::RefCell<Vec<(u64, f32)>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// Память замеров: ключ стиля и текста → положения знаков по кускам.
    static MEASURED: std::cell::RefCell<Vec<(u64, Vec<Seg>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}
