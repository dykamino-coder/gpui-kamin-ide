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

    pub(crate) fn opportunities_with_fallback(&self, hyphenate: bool) -> Vec<Stop> {
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
        out.retain(|s| {
            let w = self.wrap_at(s.at);
            // Распорка атома (U+FEFF, класс WJ) здесь не запрет: за атомом
            // точку ставит `linebreaks`.
            s.mandatory
                || w.anywhere
                || w.wrap_anywhere
                || self
                    .atom_boxes
                    .iter()
                    .any(|b| b.at + 3 == s.at || b.at == s.at)
                || self.text[..s.at].chars().next_back().is_none_or(|c| {
                    !no_break_after(c) || (w.loose >= 2 && w.cjk_lang && wide_prefix(c))
                })
        });
        out.sort_by_key(|s| (s.at, !s.mandatory));
        out.dedup_by_key(|s| s.at);
        out
    }
}
