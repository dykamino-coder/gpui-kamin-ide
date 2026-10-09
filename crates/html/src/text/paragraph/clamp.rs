//! line-clamp и text-overflow: обрезка строк, многоточие, маркеры.

use crate::text::paragraph::*;
use gpui::{Pixels, Window, px};

impl Paragraph {
    /// `line-clamp`: строк остаётся не больше заданного числа, а на последней
    /// появляется многоточие. Место под него отбирается у текста — иначе
    /// строка вылезала бы за коробку (`text-wrap-balance-line-clamp-003`).
    pub(crate) fn clamp_lines(
        &self,
        mut lines: Vec<Line>,
        limit: Option<Pixels>,
        window: &mut Window,
    ) -> Vec<Line> {
        let Some(max) = self.clamp.filter(|n| *n > 0) else {
            return lines;
        };
        // Обрывать нечего — знака нет. Исключение — авто-режим, где точка
        // среза бывает МЕЖДУ блоками: абзац видим целиком, а знак ему всё
        // равно положен, потому что за ним обрывается содержимое
        // контейнера (`line-clamp-auto-024`: срез между вторым и третьим
        // блоком, а «…» — на «Line 6»).
        if lines.len() <= max && !self.clamp_force {
            return lines;
        }
        lines.truncate(max);
        let Some(last) = lines.pop() else {
            return lines;
        };
        let segs = self.measure(window);
        let ell = self.suffix_width(self.clamp_str(), last.range.start, window);
        let head = last.range.start;
        // Висящий хвост перед знаком отбрасывается (фаза 2 css-text-3 §4.1.2),
        // но СОХРАНЁННЫЕ пробелы (`white-space: pre` куска) — содержимое:
        // знак встаёт после них (`block-ellipsis-031`).
        let trim = |at: usize| {
            let mut e = at;
            for (i, ch) in self.text[head..at].char_indices().rev() {
                let kept = matches!(ch, ' ' | '\t') && self.wrap_at(head + i).keep_spaces;
                if !kept && ch != '\u{feff}' && (hangs(ch) || zero_width(ch)) {
                    e = head + i;
                } else {
                    break;
                }
            }
            e
        };
        let mut end = trim(last.range.end.min(self.text.len()));
        // Место под многоточие отбирается ЦЕЛЫМИ кусками: строка обрывается по
        // точке переноса, а не посреди слова. Слово, которое с многоточием уже
        // не влезает, уходит со строки целиком — как в браузере.
        // `block-ellipsis: no-ellipsis` — знака нет, и место под него
        // отбирать не у чего: строка остаётся как есть, даже если её
        // непереносимое слово шире коробки (`block-ellipsis-023/024/037`).
        // Мягкий перенос — тоже точка переноса для знака (css-overflow-4
        // §block-ellipsis «as if wrapping»): разрыв на нём показывает знак
        // переноса перед многоточием (`block-ellipsis-028`: «isti‐…»).
        let shy_at = |at: usize| at > head && self.text[..at].ends_with('\u{ad}');
        let shy_w = |at: usize| {
            if shy_at(at) {
                self.hyphen_width(at)
            } else {
                px(0.)
            }
        };
        if let Some(room) = limit
            .map(|w| w - ell)
            .filter(|_| !self.clamp_str().is_empty())
            && self.span(&segs, head, end) > room
        {
            end = self
                .opportunities()
                .iter()
                .map(|s| s.at)
                .filter(|at| *at > head && *at <= end)
                .map(trim)
                .filter(|at| self.span(&segs, head, *at) + shy_w(*at) <= room)
                .max()
                .unwrap_or(head);
        }
        let hyphen = !self.hyphen.is_empty() && shy_at(end);
        let width = self.span(&segs, head, end)
            + ell
            + if hyphen {
                self.hyphen_width(end)
            } else {
                px(0.)
            };
        lines.push(Line {
            range: head..end,
            width,
            ellipsis: true,
            clamped: true,
            vis_cut: None,
            hyphen,
            indent: last.indent,
        });
        lines
    }

    /// Усечь строку под многоточие: место отбирается целыми кусками по
    /// точкам переноса — как у `line-clamp` (общая механика).
    pub(crate) fn ellipsize(
        &self,
        line: &mut Line,
        limit: Pixels,
        segs: &[Seg],
        window: &mut Window,
    ) {
        let ell = self.suffix_width(self.marker_str(), line.range.start, window);
        let head = line.range.start;
        let mut end = head + trim_hanging(&self.text[line.range.clone()]);
        let room = limit - ell;
        if let Some(cut) = self.visual_cut(head, end, room, segs) {
            line.width = cut + ell;
            line.range = head..end;
            line.ellipsis = true;
            line.vis_cut = Some(cut);
            return;
        }
        if self.wrap.rtl {
            // Письмо справа налево: строка прижата вправо, контейнер режет
            // ЛЕВЫЙ край — усечение с ЛОГИЧЕСКОГО НАЧАЛА, многоточие там же.
            let mut start = head;
            if self.span(segs, head, end) > room {
                start = self.text[head..end]
                    .char_indices()
                    .map(|(i, _)| head + i)
                    .filter(|at| *at > head && self.span(segs, *at, end) <= room)
                    .min()
                    .unwrap_or(end);
            }
            line.width = self.span(segs, start, end) + ell;
            line.range = start..end;
            line.ellipsis = true;
            return;
        }
        if self.span(segs, head, end) > room {
            let by_break = self
                .opportunities()
                .iter()
                .map(|s| s.at)
                .filter(|at| *at > head && *at <= end)
                .map(|at| head + trim_hanging(&self.text[head..at]))
                .filter(|at| self.span(segs, head, *at) <= room)
                .max();
            // Непереносимое слово режется ПО ЗНАКАМ: обрезка контейнером
            // не ждёт точки переноса (в отличие от line-clamp).
            end = by_break.unwrap_or_else(|| {
                self.text[head..end]
                    .char_indices()
                    .map(|(i, _)| head + i)
                    .filter(|at| *at > head && self.span(segs, head, *at) <= room)
                    .max()
                    // css-overflow-3 §text-overflow: «The first character or
                    // atomic inline-level element on a line must be clipped
                    // rather than ellipsed» — when not even it fits, it stays
                    // and the ellipsis follows it past the clip edge
                    // (`text-overflow-008`: a 100px glyph in a 50px box).
                    .unwrap_or_else(|| {
                        self.text[head..end]
                            .char_indices()
                            .nth(1)
                            .map_or(end, |(i, _)| head + i)
                    })
            });
        }
        line.width = self.span(segs, head, end) + ell;
        line.range = head..end;
        line.ellipsis = true;
    }

    /// Усечение строки со СМЕШАННЫМ направлением: знаки прячутся с конечного
    /// края строки в ВИДИМОМ порядке (css-overflow-3 §text-overflow:
    /// «implementations must hide characters … at the end edge of the line
    /// as necessary to fit the ellipsis»; Blink `line_truncator.cc` режет
    /// по визуальному порядку фрагментов). Логический срез резал не те
    /// знаки (`text-overflow-027/028/029`, `text-overflow-string-005…008`).
    /// Первый знак строки остаётся всегда (обрезается, а не прячется).
    /// Возвращает ширину видимой части от начального края; None — строка
    /// одного направления, ей хватает логического среза.
    pub(crate) fn visual_cut(
        &self,
        head: usize,
        end: usize,
        room: Pixels,
        segs: &[Seg],
    ) -> Option<Pixels> {
        if head >= end || end > self.text.len() || self.plaintext.is_some() {
            return None;
        }
        let base = if self.wrap.rtl {
            unicode_bidi::Level::rtl()
        } else {
            unicode_bidi::Level::ltr()
        };
        let info = unicode_bidi::BidiInfo::new(&self.text, Some(base));
        let para = info
            .paragraphs
            .iter()
            .find(|p| p.range.start <= head && head < p.range.end)?;
        let (levels, runs) = info.visual_runs(para, head..end);
        if runs.len() < 2
            && runs
                .first()
                .is_none_or(|r| levels.get(r.start) == Some(&base))
        {
            return None;
        }
        let mut order: Vec<usize> = vec![];
        for run in runs {
            let mut cs: Vec<usize> = self.text[run.clone()]
                .char_indices()
                .map(|(i, _)| run.start + i)
                .collect();
            if levels.get(run.start).is_some_and(|l| l.is_rtl()) {
                cs.reverse();
            }
            order.extend(cs);
        }
        if self.wrap.rtl {
            order.reverse();
        }
        let mut x = px(0.);
        for (n, i) in order.into_iter().enumerate() {
            let len = self.text[i..].chars().next().map_or(1, |c| c.len_utf8());
            let w = self.span(segs, i, i + len);
            if n > 0 && x + w > room {
                break;
            }
            x += w;
        }
        Some(x)
    }

    /// Ширина многоточия в наборе того куска, где оборвана строка.
    /// Маркер обрезки: свой из `text-overflow: <string>` либо многоточие.
    pub(crate) fn marker_str(&self) -> &str {
        self.overflow_marker.as_deref().unwrap_or(ELLIPSIS)
    }

    /// Знак обрыва строки. Строку, оборванную `line-clamp`, метит
    /// `block-ellipsis: auto` — это всегда U+2026 (css-overflow-4
    /// §block-ellipsis: «auto: Render an ellipsis character (U+2026)»);
    /// строка `text-overflow: <string>` относится только к обрезке по
    /// строчной оси (`line-clamp-with-text-overflow-string-001`).
    pub(crate) fn line_mark(&self, line: &Line) -> &str {
        if line.clamped {
            self.clamp_str()
        } else {
            self.marker_str()
        }
    }

    /// Знак `block-ellipsis` строки обрыва `line-clamp`.
    pub(crate) fn clamp_str(&self) -> &str {
        self.clamp_marker.as_deref().unwrap_or(ELLIPSIS)
    }

    /// Знак — анонимный строчный ребёнок блока (стиль блока, вплетается в
    /// набор строки): многоточие и строка `block-ellipsis`; строка
    /// `text-overflow` рисуется отдельно (см. `paint_line`).
    pub(crate) fn is_block_mark(&self, mark: &str) -> bool {
        !mark.is_empty()
            && (mark == ELLIPSIS || mark == self.clamp_str())
            && self.overflow_marker.as_deref() != Some(mark)
    }

    pub(crate) fn suffix_width(&self, mark: &str, at: usize, window: &mut Window) -> Pixels {
        self.shaped_suffix(mark, at, window)
            .into_iter()
            .fold(px(0.), |width, shaped| width + shaped.width)
    }
}
