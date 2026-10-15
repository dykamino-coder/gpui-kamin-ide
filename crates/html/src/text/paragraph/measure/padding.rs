//! Padding for measure; split out to keep the owning module within 250 lines.

use crate::text::paragraph::*;
use gpui::{Pixels, px};

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
                    let over = if line_no == 0 && self.ruby_trim.start {
                        0.0
                    } else {
                        b.over
                    };
                    let under = if line_no == last_line && self.ruby_trim.end {
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
                    ruby,
                    under,
                    size: h,
                    ..
                } in &self.emph_spans
                {
                    // Ruby's parent line accounts for the complete annotation
                    // layer; growing the technical base paragraph counts it twice.
                    if ruby.is_some() {
                        continue;
                    }
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
                    if (*under && line_no == last_line && self.ruby_trim.end)
                        || (!*under && line_no == 0 && self.ruby_trim.start)
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
                // `text-box-trim` первой (последней) отформатированной строки
                // режет строчную коробку до края КОРНЕВОЙ строчной коробки
                // (css-inline-3 §text-box-trim; Blink `ApplyTextBoxTrim`:
                // `line_box_metrics.ascent - intrinsic_metrics.ascent`):
                // высокое содержимое над струтом уходит в переполнение.
                // Остаток до метрики края (полулидинг струта и `cap`/`ex`)
                // снимает поле в хвосте `blocks_flow`.
                // Отсчёт — от СТРУТА, а не от базовой строки `a`: высокий
                // прогон опускает её внутри строки.
                let root = self.root_base(a);
                if line_no == 0 && self.ruby_trim.start {
                    above = above.min(self.ruby_trim.over + root - a);
                }
                if line_no == last_line && self.ruby_trim.end {
                    below = below.min(a - root);
                }
                (above, below)
            })
            .collect()
    }
}

impl Paragraph {
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
}

impl Paragraph {
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
            // css-text-3 9.2 `first`: Ps/Pi/Pf, ASCII quotes and the
            // IDEOGRAPHIC SPACE U+3000 (`hanging-punctuation-first-002`).
            Some(ch) if is_opening(ch) || ch == '\u{3000}' => ch.len_utf8(),
            _ => 0,
        }
    }
}

impl Paragraph {
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
}
