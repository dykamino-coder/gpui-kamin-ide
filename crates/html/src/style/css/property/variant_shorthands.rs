//! Сокращения вариантов шрифта, раскрытые при разборе: `font-variant`
//! (css-fonts-4 §6.11) и `font-synthesis` (§font-synthesis). Слова
//! `font-variant` разносятся по своим `font-variant-*` в любом порядке (Blink
//! `FontVariantShorthandParser`); опущенное свойство — `normal`.

use super::font_shorthands::{FONT, words};
use super::{LonghandId, ShorthandId};
use LonghandId::*;

pub(super) const FONT_VARIANT: &[LonghandId] = &[
    FontVariantLigatures,
    FontVariantCaps,
    FontVariantAlternates,
    FontVariantNumeric,
    FontVariantEastAsian,
    FontVariantPosition,
    FontVariantEmoji,
];

pub(super) const FONT_SYNTHESIS: &[LonghandId] = &[
    FontSynthesisWeight,
    FontSynthesisStyle,
    FontSynthesisSmallCaps,
    FontSynthesisPosition,
];

/// Длинное свойство `font-variant-*` слова значения `font-variant`.
fn variant_of(word: &str) -> Option<LonghandId> {
    let w = word.to_ascii_lowercase();
    let fun = |f: &str| w.starts_with(f) && w[f.len()..].starts_with('(');
    Some(match w.as_str() {
        "common-ligatures"
        | "no-common-ligatures"
        | "discretionary-ligatures"
        | "no-discretionary-ligatures"
        | "historical-ligatures"
        | "no-historical-ligatures"
        | "contextual"
        | "no-contextual" => FontVariantLigatures,
        "small-caps" | "all-small-caps" | "petite-caps" | "all-petite-caps" | "unicase"
        | "titling-caps" => FontVariantCaps,
        "lining-nums" | "oldstyle-nums" | "proportional-nums" | "tabular-nums"
        | "diagonal-fractions" | "stacked-fractions" | "ordinal" | "slashed-zero" => {
            FontVariantNumeric
        }
        "jis78" | "jis83" | "jis90" | "jis04" | "simplified" | "traditional" | "full-width"
        | "proportional-width" | "ruby" => FontVariantEastAsian,
        "sub" | "super" => FontVariantPosition,
        "text" | "emoji" | "unicode" => FontVariantEmoji,
        "historical-forms" => FontVariantAlternates,
        _ if [
            "stylistic",
            "styleset",
            "character-variant",
            "swash",
            "ornaments",
            "annotation",
        ]
        .iter()
        .any(|f| fun(f)) =>
        {
            FontVariantAlternates
        }
        _ => return None,
    })
}

/// `font-variant: normal | none | [ <lig> || <caps> || <alternates> ||
/// <numeric> || <east-asian> || <position> || <emoji> ]`: слова разносятся
/// по своим свойствам (в любом порядке, как Blink), повтор внутри свойства
/// отвергает его разбор. `none` — лигатуры `none`, остальное `normal`.
pub(super) fn font_variant(value: &str) -> Option<Vec<String>> {
    let v = value.trim();
    let mut parts: Vec<String> = FONT_VARIANT.iter().map(|_| String::new()).collect();
    match v.to_ascii_lowercase().as_str() {
        "normal" => {}
        "none" => parts[0] = "none".into(),
        _ => {
            for (s, e) in words(v)? {
                let id = variant_of(&v[s..e])?;
                let slot = FONT_VARIANT.iter().position(|&l| l == id)?;
                if !parts[slot].is_empty() {
                    parts[slot].push(' ');
                }
                parts[slot].push_str(&v[s..e]);
            }
        }
    }
    Some(
        parts
            .into_iter()
            .map(|p| if p.is_empty() { "normal".into() } else { p })
            .collect(),
    )
}

/// `font-synthesis: none | [ weight || style || small-caps || position ]`:
/// названное — `auto`, прочее — `none`.
pub(super) fn font_synthesis(value: &str) -> Option<Vec<String>> {
    let v = value.trim().to_ascii_lowercase();
    let mut on = [false; 4];
    if v != "none" {
        for w in v.split_whitespace() {
            let i = ["weight", "style", "small-caps", "position"]
                .iter()
                .position(|n| *n == w)?;
            if std::mem::replace(&mut on[i], true) {
                return None;
            }
        }
    }
    Some(
        on.iter()
            .map(|&a| if a { "auto" } else { "none" }.to_string())
            .collect(),
    )
}

impl ShorthandId {
    /// Длинные свойства сокращений шрифта.
    pub(super) fn font_longhands(self) -> Option<&'static [LonghandId]> {
        Some(match self {
            Self::Font => FONT,
            Self::FontVariant => FONT_VARIANT,
            Self::FontSynthesis => FONT_SYNTHESIS,
            _ => return None,
        })
    }
}
