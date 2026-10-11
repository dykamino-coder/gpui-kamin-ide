//! Ellipsis for clamp; split out to keep the owning module within 250 lines.

use crate::text::paragraph::*;
use gpui::{Pixels, Window, px};

impl Paragraph {
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
}

impl Paragraph {
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
}

impl Paragraph {
    /// Ширина многоточия в наборе того куска, где оборвана строка.
    /// Маркер обрезки: свой из `text-overflow: <string>` либо многоточие.
    pub(crate) fn marker_str(&self) -> &str {
        self.overflow_marker.as_deref().unwrap_or(ELLIPSIS)
    }
}

impl Paragraph {
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
}

impl Paragraph {
    /// Знак `block-ellipsis` строки обрыва `line-clamp`.
    pub(crate) fn clamp_str(&self) -> &str {
        self.clamp_marker.as_deref().unwrap_or(ELLIPSIS)
    }
}

impl Paragraph {
    /// Знак — анонимный строчный ребёнок блока (стиль блока, вплетается в
    /// набор строки): многоточие и строка `block-ellipsis`; строка
    /// `text-overflow` рисуется отдельно (см. `paint_line`).
    pub(crate) fn is_block_mark(&self, mark: &str) -> bool {
        !mark.is_empty()
            && (mark == ELLIPSIS || mark == self.clamp_str())
            && self.overflow_marker.as_deref() != Some(mark)
    }
}

impl Paragraph {
    pub(crate) fn suffix_width(&self, mark: &str, at: usize, window: &mut Window) -> Pixels {
        self.shaped_suffix(mark, at, window)
            .into_iter()
            .fold(px(0.), |width, shaped| width + shaped.width)
    }
}
