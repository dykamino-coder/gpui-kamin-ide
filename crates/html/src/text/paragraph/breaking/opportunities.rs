//! Итоговый список точек разрыва абзаца (`opportunities`): обязательные
//! разрывы по `\n`, мягкие точки из `soft_opportunities`, затем фильтры —
//! мягкие переносы `auto-phrase` только как запасной вариант, `nowrap`/`pre`
//! по общему предку двух знаков (css-text-3 §5.1), границы гроздей знаков с
//! учётом `line-break: anywhere`, запреты после знака-приставки перед числом
//! (кроме широких префиксов при `line-break: loose` в ja/zh, css-text-3 §5.2).

use super::{cluster_edge_at, no_break_after, wide_prefix};
use crate::text::paragraph::*;

impl Paragraph {
    pub(crate) fn opportunities(&self) -> Vec<Stop> {
        self.opportunities_with_fallback(false)
    }

    /// Break opportunities depend on the text and its wrap rules only, never
    /// on the available width: they are found once per paragraph content and
    /// kept by the paragraph's preparation and, across probes, phases and
    /// frames, by the document (`MeasureCache::stops`). A width change then
    /// only re-runs line breaking over the cached opportunities and widths.
    pub(crate) fn opportunities_with_fallback(&self, hyphenate: bool) -> Vec<Stop> {
        if self.text.len() > (u32::MAX >> 1) as usize {
            return self.opportunities_uncached(hyphenate);
        }
        let packed = self.prepared().stops[usize::from(hyphenate)].get_or_init(|| {
            // Live atoms (ruby bases) take part in the breaks through their
            // slots, which no content key covers: those stay per paragraph.
            let key = self.atoms.is_empty().then(|| self.stops_key(hyphenate));
            if let Some(key) = key
                && let Some(hit) = self.measures.stops.borrow_mut().get(&key)
            {
                return hit.clone();
            }
            let packed: std::rc::Rc<[u32]> = self
                .opportunities_uncached(hyphenate)
                .iter()
                .map(|s| (s.at as u32) << 1 | u32::from(s.mandatory))
                .collect();
            if let Some(key) = key {
                self.measures.stops.borrow_mut().insert(key, packed.clone());
            }
            packed
        });
        packed
            .iter()
            .map(|&s| Stop {
                at: (s >> 1) as usize,
                mandatory: s & 1 == 1,
            })
            .collect()
    }

    /// Everything `opportunities_uncached` reads, apart from live atoms.
    fn stops_key(&self, hyphenate: bool) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.text_hash().hash(&mut h);
        self.wrap.hash(&mut h);
        self.spans.hash(&mut h);
        self.spacers.hash(&mut h);
        self.atom_boxes.len().hash(&mut h);
        for b in &self.atom_boxes {
            b.at.hash(&mut h);
        }
        hyphenate.hash(&mut h);
        h.finish()
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
    use std::rc::Rc;

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
            .iter()
            .map(|s| (s.at, s.mandatory))
            .collect()
    }

    #[test]
    fn opportunities_are_found_once_per_content_and_shared_by_copies() {
        let p = paragraph("aa bb\ncc dd", Wrap::default());
        assert_eq!(at(&p, false), [(3, false), (6, true), (9, false)]);
        assert_eq!(
            at(&p, false),
            p.opportunities_uncached(false)
                .iter()
                .map(|s| (s.at, s.mandatory))
                .collect::<Vec<_>>()
        );
        // A copy of the same content (a layout probe, the next frame's
        // paragraph) takes the document's entry instead of breaking again.
        let marker: Rc<[u32]> = Rc::from([7 << 1 | 1]);
        p.measures
            .stops
            .borrow_mut()
            .insert(p.stops_key(false), marker);
        let mut copy = paragraph("aa bb\ncc dd", Wrap::default());
        copy.measures = p.measures.clone();
        assert_eq!(at(&copy, false), [(7, true)]);
        // Other wrap rules or the hyphenation fallback are other entries.
        let mut other = paragraph(
            "aa bb\ncc dd",
            Wrap {
                break_all: true,
                ..Default::default()
            },
        );
        other.measures = p.measures.clone();
        assert_ne!(at(&other, false), [(7, true)]);
        assert_ne!(at(&copy, true), [(7, true)]);
        assert_ne!(p.stops_key(false), p.stops_key(true));
        assert_ne!(
            p.stops_key(false),
            paragraph("aa bb\ncc dd", Wrap::default())
                .spacers(vec![3])
                .stops_key(false)
        );
    }
}
