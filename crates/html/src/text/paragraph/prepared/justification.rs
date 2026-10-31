//! Точки раздачи свободного места: границы графем и предыдущий видимый
//! знак готовятся один раз; начало строки ограничивает только поиск.

use super::super::*;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

pub(in crate::text::paragraph) struct Opportunities {
    words: Vec<usize>,
    boundaries: Vec<(usize, usize)>,
}

impl Opportunities {
    pub(super) fn new(p: &Paragraph) -> Self {
        let mut words = Vec::new();
        let mut boundaries = Vec::new();
        let mut previous = None;
        let mut units = p.text.grapheme_indices(true).map(|(at, _)| at).peekable();
        let plain =
            |c| !word_separator(c) && !invisible(c) && !matches!(c, '\t' | '\n' | '\u{3000}');
        for (at, b) in p.text.char_indices() {
            while units.peek().is_some_and(|&i| i < at) {
                units.next();
            }
            if word_separator(b) && !p.ruby_justify {
                words.push(at);
            }
            if p.justify_chars != 0
                && !p.ruby_justify
                && !p.ruby_unit
                && let Some((before, a)) = previous
                && plain(a)
                && plain(b)
                && cluster_edge(&p.text, at)
                && units.peek() == Some(&at)
                && if p.justify_chars == 1 {
                    justify_ideograph(a) || justify_ideograph(b)
                } else {
                    !(cursive_script(a) && cursive_script(b))
                }
            {
                boundaries.push((at, before));
            }
            if !invisible(b) {
                previous = Some((at, b));
            }
        }
        Self { words, boundaries }
    }

    pub(in crate::text::paragraph) fn boundary(&self, start: usize, at: usize) -> bool {
        let i = self.boundaries.partition_point(|b| b.0 < at);
        self.boundaries
            .get(i)
            .is_some_and(|b| b.0 == at && b.1 >= start)
    }

    pub(in crate::text::paragraph) fn count(&self, range: &Range<usize>, pos: usize) -> usize {
        let pos = pos.clamp(range.start, range.end);
        let words = self.words.partition_point(|&at| at < pos)
            - self.words.partition_point(|&at| at < range.start);
        let first = self.boundaries.partition_point(|b| b.1 < range.start);
        let last = self.boundaries.partition_point(|b| {
            if pos < range.end {
                b.0 <= pos
            } else {
                b.0 < pos
            }
        });
        words + last.saturating_sub(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    fn original(p: &Paragraph, start: usize, at: usize) -> bool {
        if p.justify_chars == 0
            || p.ruby_justify
            || p.ruby_unit
            || at <= start
            || at >= p.text.len()
        {
            return false;
        }
        let b = p.text[at..].chars().next().unwrap();
        let Some(a) = p.text[start..at].chars().rev().find(|c| !invisible(*c)) else {
            return false;
        };
        let plain =
            |c| !word_separator(c) && !invisible(c) && !matches!(c, '\t' | '\n' | '\u{3000}');
        plain(a)
            && plain(b)
            && cluster_edge(&p.text, at)
            && unicode_segmentation::GraphemeCursor::new(at, p.text.len(), true)
                .is_boundary(&p.text, 0)
                .unwrap_or(false)
            && if p.justify_chars == 1 {
                justify_ideograph(a) || justify_ideograph(b)
            } else {
                !(cursive_script(a) && cursive_script(b))
            }
    }

    #[test]
    fn indexed_opportunities_match_all_line_ranges() {
        for text in [
            "東\u{2066}京 西\t南\n北",
            "a\u{301}b👩‍💻🇺🇿 c",
            "اَب ج東京",
            "x\u{200b}\u{2066}y",
        ] {
            for mode in 0..=2 {
                let p = Paragraph::new(
                    text.into(),
                    vec![],
                    px(16.),
                    px(20.),
                    Align::Justify,
                    Wrap::default(),
                )
                .justify_chars(mode);
                let index = Opportunities::new(&p);
                let points: Vec<_> = text
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain([text.len()])
                    .collect();
                for &start in &points {
                    for &end in points.iter().filter(|&&i| i >= start) {
                        for &pos in points.iter().filter(|&&i| i >= start && i <= end) {
                            assert_eq!(index.boundary(start, pos), original(&p, start, pos));
                            let expected = text[start..pos]
                                .char_indices()
                                .filter(|&(i, c)| {
                                    word_separator(c) || original(&p, start, start + i)
                                })
                                .count()
                                + usize::from(pos < end && original(&p, start, pos));
                            assert_eq!(
                                index.count(&(start..end), pos),
                                expected,
                                "{text:?} {start}..{end} at {pos}"
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn ruby_flags_suppress_only_the_original_opportunities() {
        for (ruby_unit, ruby_justify) in [(true, false), (false, true), (true, true)] {
            let mut p = Paragraph::new(
                "東 京".into(),
                vec![],
                px(16.),
                px(20.),
                Align::Justify,
                Wrap::default(),
            )
            .justify_chars(2);
            p.ruby_unit = ruby_unit;
            p.ruby_justify = ruby_justify;
            let index = Opportunities::new(&p);
            assert!(!index.boundary(0, 3));
            assert_eq!(
                index.count(&(0..p.text.len()), p.text.len()),
                usize::from(!ruby_justify)
            );
        }
    }
}
