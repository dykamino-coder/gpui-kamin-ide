//! Разрешённые уровни UAX9 принадлежат абзацу; L1/L2 обрабатывают только
//! байты запрошенной строки, не клонируя уровни всего абзаца на каждый кусок.

use std::ops::Range;
use unicode_bidi::{BidiClass, BidiInfo, Level, ParagraphBidiInfo, ParagraphInfo};

pub(super) type VisualRuns = (Level, Vec<(usize, usize, bool)>);

pub(super) struct ResolvedBidi {
    classes: Vec<BidiClass>,
    levels: Vec<Level>,
    paragraphs: Vec<ParagraphInfo>,
}

impl ResolvedBidi {
    pub(super) fn new(text: &str, forced: Option<Level>) -> Self {
        let info = BidiInfo::new(text, forced);
        Self {
            classes: info.original_classes,
            levels: info.levels,
            paragraphs: info.paragraphs,
        }
    }

    pub(super) fn visual(&self, text: &str, range: &Range<usize>) -> Option<VisualRuns> {
        let p = self
            .paragraphs
            .partition_point(|p| p.range.end <= range.start);
        let para = self.paragraphs.get(p).or_else(|| self.paragraphs.first())?;
        if range.is_empty() {
            return Some((para.level, Vec::new()));
        }
        let local = ParagraphBidiInfo {
            text: &text[range.clone()],
            original_classes: self.classes[range.clone()].to_vec(),
            levels: self.levels[range.clone()].to_vec(),
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
        ] {
            for forced in [None, Some(Level::ltr()), Some(Level::rtl())] {
                let full = BidiInfo::new(text, forced);
                let prepared = ResolvedBidi::new(text, forced);
                for para in &full.paragraphs {
                    let boundaries: Vec<_> = (para.range.start..=para.range.end)
                        .filter(|&i| text.is_char_boundary(i))
                        .collect();
                    for &start in &boundaries {
                        for &end in boundaries.iter().filter(|&&e| e > start) {
                            let (levels, visual) = full.visual_runs(para, start..end);
                            let expected: Vec<_> = visual
                                .into_iter()
                                .map(|r| (r.start, r.end, levels[r.start].is_rtl()))
                                .collect();
                            assert_eq!(
                                prepared.visual(text, &(start..end)),
                                Some((para.level, expected)),
                                "{text:?} {start}..{end}"
                            );
                        }
                    }
                }
            }
        }
    }
}
