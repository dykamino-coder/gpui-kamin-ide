//! line-clamp и text-overflow: обрезка строк, многоточие, маркеры.

mod ellipsis;

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
}
