//! Разрешённые уровни UAX9 принадлежат абзацу; L1/L2 обрабатывают только
//! байты запрошенной строки, не клонируя уровни всего абзаца на каждый кусок.

use std::ops::Range;
use unicode_bidi::{BidiClass, BidiInfo, Level, ParagraphBidiInfo, ParagraphInfo};

pub(super) type VisualRuns = (Level, Vec<(usize, usize, bool)>);

pub(super) enum ResolvedBidi {
    /// Text without right-to-left, Arabic-number or explicit embedding,
    /// override and isolate characters in a left-to-right (or auto) base
    /// direction: UAX #9 resolves every character to level 0 (W7 turns
    /// European numbers after the L start of sequence into L, N1/N2 give
    /// neutrals between L and sos/eos level 0), so every line is one LTR
    /// run. Blink skips its bidi pass for such text the same way.
    Ltr { empty: bool },
    Full {
        classes: Vec<BidiClass>,
        levels: Vec<Level>,
        paragraphs: Vec<ParagraphInfo>,
    },
}

/// Whether the text has a character after which UAX #9 can resolve a level
/// other than 0 or split a line into several runs.
fn needs_bidi(text: &str) -> bool {
    !text.is_ascii()
        && text.chars().any(|c| {
            use BidiClass::*;
            matches!(
                unicode_bidi::bidi_class(c),
                R | AL | AN | RLE | RLO | RLI | LRE | LRO | LRI | FSI | PDF | PDI
            )
        })
}

impl ResolvedBidi {
    pub(super) fn new(text: &str, forced: Option<Level>) -> Self {
        if forced.is_none_or(|l| l.is_ltr()) && !needs_bidi(text) {
            return Self::Ltr {
                empty: text.is_empty(),
            };
        }
        let info = BidiInfo::new(text, forced);
        Self::Full {
            classes: info.original_classes,
            levels: info.levels,
            paragraphs: info.paragraphs,
        }
    }

    pub(super) fn visual(&self, text: &str, range: &Range<usize>) -> Option<VisualRuns> {
        let (classes, levels, paragraphs) = match self {
            // UAX #9 reports no paragraph for empty text.
            Self::Ltr { empty: true } => return None,
            Self::Ltr { empty: false } if range.is_empty() => {
                return Some((Level::ltr(), Vec::new()));
            }
            Self::Ltr { empty: false } => {
                return Some((Level::ltr(), vec![(range.start, range.end, false)]));
            }
            Self::Full {
                classes,
                levels,
                paragraphs,
            } => (classes, levels, paragraphs),
        };
        let p = paragraphs.partition_point(|p| p.range.end <= range.start);
        let para = paragraphs.get(p).or_else(|| paragraphs.first())?;
        if range.is_empty() {
            return Some((para.level, Vec::new()));
        }
        let local = ParagraphBidiInfo {
            text: &text[range.clone()],
            original_classes: classes[range.clone()].to_vec(),
            levels: levels[range.clone()].to_vec(),
            paragraph_level: para.level,
            is_pure_ltr: false,
        };
        let (levels, runs) = local.visual_runs(0..range.len());
        Some((
            para.level,
            runs.into_iter()
                .map(|r| {
                    (
                        range.start + r.start,
                        range.start + r.end,
                        levels[r.start].is_rtl(),
                    )
                })
                .collect(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_local_reordering_matches_full_paragraph() {
        for text in [
            "abc אבג 12  \n xyz",
            "א\u{2066}abc\u{2069} ב",
            "x\u{202e}12\u{202c} \t",
            "\u{fffc}אבג\u{fffc}",
            // Left-to-right text takes the level-0 shortcut.
            "abc 12, 3.5% x\ty  \n 7 z ",
            "\u{fffc}é — «ü» 1\u{2029}2 \u{feff}q\u{200b}",
            " \t12 ",
        ] {
            for forced in [None, Some(Level::ltr()), Some(Level::rtl())] {
                let full = BidiInfo::new(text, forced);
                let prepared = ResolvedBidi::new(text, forced);
                for para in &full.paragraphs {
                    let boundaries: Vec<_> = (para.range.start..=para.range.end)
                        .filter(|&i| text.is_char_boundary(i))
                        .collect();
                    for &start in &boundaries {
                        assert_eq!(
                            prepared.visual(text, &(start..start)),
                            Some((para.level, Vec::new()))
                        );
                        for &end in boundaries.iter().filter(|&&e| e > start) {
                            let (levels, visual) = full.visual_runs(para, start..end);
                            let expected: Vec<_> = visual
                                .into_iter()
                                .map(|r| (r.start, r.end, levels[r.start].is_rtl()))
                                .collect();
                            assert_eq!(
                                prepared.visual(text, &(start..end)),
                                Some((para.level, expected)),
                                "{text:?} {forced:?} {start}..{end}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn only_left_to_right_text_skips_resolution() {
        let ltr = |text: &str, forced| {
            matches!(ResolvedBidi::new(text, forced), ResolvedBidi::Ltr { .. })
        };
        assert!(ltr("abc 12", None));
        assert!(ltr("é ü 1", Some(Level::ltr())));
        assert!(!ltr("abc", Some(Level::rtl())));
        assert!(!ltr("a ב", None));
        assert!(!ltr("a \u{0661}", None));
        assert!(!ltr("a\u{2068}b\u{2069}", None));
        assert_eq!(ResolvedBidi::new("", None).visual("", &(0..0)), None);
    }
}
