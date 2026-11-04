//! Итоговый список точек разрыва абзаца (`opportunities`): обязательные
//! разрывы по `\n`, мягкие точки из `soft_opportunities`, затем фильтры —
//! мягкие переносы `auto-phrase` только как запасной вариант, `nowrap`/`pre`
//! по общему предку двух знаков (css-text-3 §5.1), границы гроздей знаков с
//! учётом `line-break: anywhere`, запреты после знака-приставки перед числом
//! (кроме широких префиксов при `line-break: loose` в ja/zh, css-text-3 §5.2).

use super::{cluster_edge_at, no_break_after, wide_prefix};
use crate::text::paragraph::*;

/// Break opportunities of a paragraph in text order: a borrowed view of the
/// list its preparation keeps, or a list found for this call only.
pub(crate) enum Stops<'a> {
    Kept(std::slice::Iter<'a, u32>),
    Found(std::vec::IntoIter<Stop>),
}

fn unpack(s: u32) -> Stop {
    Stop {
        at: (s >> 1) as usize,
        mandatory: s & 1 == 1,
    }
}

impl Iterator for Stops<'_> {
    type Item = Stop;
    fn next(&mut self) -> Option<Stop> {
        match self {
            Self::Kept(kept) => kept.next().map(|&s| unpack(s)),
            Self::Found(found) => found.next(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Kept(kept) => kept.size_hint(),
            Self::Found(found) => found.size_hint(),
        }
    }
}

impl ExactSizeIterator for Stops<'_> {}

impl Paragraph {
    pub(crate) fn opportunities(&self) -> Stops<'_> {
        self.opportunities_with_fallback(false)
    }

    /// Break opportunities depend on the text and its wrap rules only, never
    /// on the available width: the paragraph's preparation finds them once
    /// and every split of the paragraph (min-content, each layout limit,
    /// prepaint) reads the same packed list without copying it. The
    /// hyphenation fallback runs only when a word overflows the line and is
    /// not kept.
    pub(crate) fn opportunities_with_fallback(&self, hyphenate: bool) -> Stops<'_> {
        match self.kept_stops() {
            Some(kept) if !hyphenate => Stops::Kept(kept.iter()),
            _ => Stops::Found(self.opportunities_uncached(hyphenate).into_iter()),
        }
    }

    /// The preparation's packed list (`offset << 1 | mandatory`, ascending
    /// offsets); `None` for text too long to pack.
    fn kept_stops(&self) -> Option<&[u32]> {
        if self.text.len() > (u32::MAX >> 1) as usize {
            return None;
        }
        Some(self.prepared().stops.get_or_init(|| {
            let found = self.opportunities_uncached(false);
            let mut kept = Vec::with_capacity(found.len());
            kept.extend(
                found
                    .iter()
                    .map(|s| (s.at as u32) << 1 | u32::from(s.mandatory)),
            );
            kept.into_boxed_slice()
        }))
    }

    /// Opportunities after `part.start` up to and including `part.end`, in
    /// text order: a binary search of the kept list, one exact allocation.
    pub(crate) fn opportunities_in(&self, part: &std::ops::Range<usize>) -> Vec<Stop> {
        let Some(kept) = self.kept_stops() else {
            return self
                .opportunities_uncached(false)
                .into_iter()
                .filter(|s| s.at > part.start && s.at <= part.end)
                .collect();
        };
        let after = |at: usize| kept.partition_point(|&s| (s >> 1) as usize <= at);
        let (lo, hi) = (after(part.start), after(part.end));
        // One more for the end-of-part check the line loop appends.
        let mut out = Vec::with_capacity(hi - lo + 1);
        out.extend(kept[lo..hi].iter().map(|&s| unpack(s)));
        out
    }
    fn opportunities_uncached(&self, hyphenate: bool) -> Vec<Stop> {
        let mut out: Vec<Stop> = Vec::new();
        // Обязательные разрывы есть всегда, даже при `nowrap`.
        for (i, ch) in self.text.char_indices() {
            if ch == '\n' {
                out.push(Stop {
                    at: i + 1,
                    mandatory: true,
                });
            }
        }
        self.soft_opportunities(&mut out);
        // CSS Text 4: auto-phrase suppresses hyphenation for normal layout
        // and intrinsic sizing, but retains it as an overflow fallback.
        if !hyphenate {
            out.retain(|s| {
                s.mandatory
                    || !self.wrap_at(s.at.saturating_sub(1)).auto_phrase
                    || !self.text[..s.at].ends_with(SOFT_HYPHEN)
            });
        }
        // `white-space: nowrap`/`pre` гасит МЯГКИЕ точки — но по месту, а не по
        // абзацу целиком: вложенный `<span>` со своим `white-space` переносится
        // внутри неперносимого абзаца и наоборот (`white-space-pre-031`).
        //
        // Точку между ДВУМЯ знаками решает их общий предок (css-text-3
        // §5.1), поэтому гасится она, только если неперносимы ОБЕ стороны.
        // Пока смотрели один знак слева, `<span style="white-space:pre">口</span>口`
        // не рвался на границе куска, хотя рвать там велит div-родитель
        // (`line-breaking-ic-001`).
        out.retain(|s| s.mandatory || !self.nowrap_between(s.at));
        // Внутри грозди знаков рвать нельзя НИКОГДА: составной знак (флаг,
        // смайлик с модификатором) — одна буква, и переносчик UAX-14 о его
        // устройстве не знает (`line-breaking-014`: радужный флаг рассыпался
        // на четыре строки).
        // Обязательный разрыв не снимается НИКОГДА: перевод строки обязан
        // закончить строку, иначе набор получает строку с переводом внутри и
        // падает на проверке (`text argument should not contain newlines`).
        // `line-break: anywhere` рвёт между ЛЮБЫМИ знаками, включая склеенные
        // соединителем нулевой ширины: класс ZWJ он тоже перекрывает
        // (`line-break-anywhere-overrides-uax-behavior-015`).
        out.retain(|s| {
            s.mandatory || cluster_edge_at(&self.text, s.at, self.wrap_at(s.at).anywhere)
        });
        // Знак перед числом держит следующий за собой: `$`, `£`, `\`. Таблица
        // пар UAX-14 в переносчике этого не знает и рвёт «XX XX\\\» между
        // обратными косыми (`word-break-break-all-023`), тогда как рвать
        // разрешено только ПЕРЕД первой из них.
        // `line-break: anywhere` и `overflow-wrap: anywhere` снимают запреты
        // типографики целиком — их точки остаются.
        // `line-break: loose` в китайском/японском письме снимает запрет и
        // после ШИРОКИХ префиксов (css-text-3 §5.2: «breaks after prefixes
        // (PR) with East Asian Width A/F/W», `line-break-loose-018`).
        // Распорка атома (U+FEFF, класс WJ) здесь не запрет: за атомом
        // точку ставит `linebreaks`. Принадлежность — по индексу коробок,
        // а не перебором всех атомов на каждую точку.
        let atoms = &self.prepared().atoms;
        let atom_at = |at: usize| atoms.first(at).is_some();
        out.retain(|s| {
            let w = self.wrap_at(s.at);
            s.mandatory
                || w.anywhere
                || w.wrap_anywhere
                || atom_at(s.at)
                || s.at.checked_sub(3).is_some_and(atom_at)
                || self.text[..s.at].chars().next_back().is_none_or(|c| {
                    !no_break_after(c) || (w.loose >= 2 && w.cjk_lang && wide_prefix(c))
                })
        });
        out.sort_by_key(|s| (s.at, !s.mandatory));
        out.dedup_by_key(|s| s.at);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    fn paragraph(text: &str, wrap: Wrap) -> Paragraph {
        Paragraph::new(
            text.to_owned().into(),
            vec![],
            px(16.),
            px(20.),
            Align::Left,
            wrap,
        )
    }

    fn at(p: &Paragraph, hyphenate: bool) -> Vec<(usize, bool)> {
        p.opportunities_with_fallback(hyphenate)
            .map(|s| (s.at, s.mandatory))
            .collect()
    }

    #[test]
    fn opportunities_are_found_once_per_preparation() {
        let p = paragraph("aa bb\ncc dd", Wrap::default());
        let uncached = |p: &Paragraph, hyphenate| -> Vec<(usize, bool)> {
            p.opportunities_uncached(hyphenate)
                .iter()
                .map(|s| (s.at, s.mandatory))
                .collect()
        };
        assert_eq!(at(&p, false), [(3, false), (6, true), (9, false)]);
        assert!(
            p.prepared().stops.get().is_some(),
            "kept by the preparation"
        );
        assert_eq!(at(&p, false), uncached(&p, false));
        assert_eq!(at(&p, true), uncached(&p, true));
        // Wrap rules and spacers are inputs of the preparation: a paragraph
        // built with others breaks anew.
        let spaced = paragraph("aa bb\ncc dd", Wrap::default()).spacers(vec![3]);
        assert_eq!(at(&spaced, false), uncached(&spaced, false));
        let all = paragraph(
            "aa bb\ncc dd",
            Wrap {
                break_all: true,
                ..Default::default()
            },
        );
        assert_eq!(at(&all, false), uncached(&all, false));
        assert_ne!(at(&all, false), at(&p, false));
    }
}
