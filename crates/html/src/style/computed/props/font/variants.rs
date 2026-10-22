//! `font-variant-*` (css-fonts-4 §6.4-§6.7, §6.10): грамматика каждого
//! длинного свойства и его группа тегов OpenType. Свойство владеет ТОЛЬКО
//! своей группой: запись заменяет её, а наследуется каждая группа отдельно
//! (`Computed::font_variant_set`).

use crate::style::css::LonghandId;

/// Группы тегов по свойствам: лигатуры, прописные, цифры, восточноазиатские,
/// положение. Индекс группы — разряд в `font_variant_set`.
pub(crate) const GROUP_TAGS: [&[&str]; 5] = [
    &["liga", "clig", "dlig", "hlig", "calt"],
    &["smcp", "c2sc", "pcap", "c2pc", "unic", "titl"],
    &[
        "lnum", "onum", "pnum", "tnum", "frac", "afrc", "ordn", "zero",
    ],
    &[
        "jp78", "jp83", "jp90", "jp04", "smpl", "trad", "fwid", "pwid", "ruby",
    ],
    &["subs", "sups"],
];

/// Группа тегов свойства (`None` — свойство без тегов в `font_features`).
pub(crate) fn variant_group(id: LonghandId) -> Option<usize> {
    use LonghandId::*;
    Some(match id {
        FontVariantLigatures => 0,
        FontVariantCaps => 1,
        FontVariantNumeric => 2,
        FontVariantEastAsian => 3,
        FontVariantPosition => 4,
        _ => return None,
    })
}

/// Подгруппы `||` значения свойства: из каждой — не больше одного слова.
fn subgroups(id: LonghandId) -> &'static [&'static [&'static str]] {
    use LonghandId::*;
    match id {
        FontVariantLigatures => &[
            &["common-ligatures", "no-common-ligatures"],
            &["discretionary-ligatures", "no-discretionary-ligatures"],
            &["historical-ligatures", "no-historical-ligatures"],
            &["contextual", "no-contextual"],
        ],
        FontVariantNumeric => &[
            &["lining-nums", "oldstyle-nums"],
            &["proportional-nums", "tabular-nums"],
            &["diagonal-fractions", "stacked-fractions"],
            &["ordinal"],
            &["slashed-zero"],
        ],
        FontVariantEastAsian => &[
            &[
                "jis78",
                "jis83",
                "jis90",
                "jis04",
                "simplified",
                "traditional",
            ],
            &["full-width", "proportional-width"],
            &["ruby"],
        ],
        _ => &[],
    }
}

/// Теги OpenType слова значения.
fn tags(word: &str) -> &'static [(&'static str, u32)] {
    match word {
        "small-caps" => &[("smcp", 1)],
        "all-small-caps" => &[("smcp", 1), ("c2sc", 1)],
        "petite-caps" => &[("pcap", 1)],
        "all-petite-caps" => &[("pcap", 1), ("c2pc", 1)],
        "unicase" => &[("unic", 1)],
        "titling-caps" => &[("titl", 1)],
        "lining-nums" => &[("lnum", 1)],
        "oldstyle-nums" => &[("onum", 1)],
        "proportional-nums" => &[("pnum", 1)],
        "tabular-nums" => &[("tnum", 1)],
        "diagonal-fractions" => &[("frac", 1)],
        "stacked-fractions" => &[("afrc", 1)],
        "ordinal" => &[("ordn", 1)],
        "slashed-zero" => &[("zero", 1)],
        "common-ligatures" => &[("liga", 1), ("clig", 1)],
        "no-common-ligatures" => &[("liga", 0), ("clig", 0)],
        "discretionary-ligatures" => &[("dlig", 1)],
        "no-discretionary-ligatures" => &[("dlig", 0)],
        "historical-ligatures" => &[("hlig", 1)],
        "no-historical-ligatures" => &[("hlig", 0)],
        "contextual" => &[("calt", 1)],
        "no-contextual" => &[("calt", 0)],
        "jis78" => &[("jp78", 1)],
        "jis83" => &[("jp83", 1)],
        "jis90" => &[("jp90", 1)],
        "jis04" => &[("jp04", 1)],
        "simplified" => &[("smpl", 1)],
        "traditional" => &[("trad", 1)],
        "full-width" => &[("fwid", 1)],
        "proportional-width" => &[("pwid", 1)],
        "ruby" => &[("ruby", 1)],
        "sub" => &[("subs", 1)],
        "super" => &[("sups", 1)],
        // `font-variant-ligatures: none` выключает и лигатуры по умолчанию
        // (§6.4).
        "none" => &[("liga", 0), ("clig", 0), ("calt", 0)],
        _ => &[],
    }
}

/// Теги значения длинного свойства `font-variant-*` с тегами; `None` — значение
/// негодно. `normal` — пустой список (теги группы по умолчанию шрифта).
pub(crate) fn variant_tags(id: LonghandId, v: &str) -> Option<Vec<(&'static str, u32)>> {
    use LonghandId::*;
    let v = v.trim().to_ascii_lowercase();
    if v == "normal" {
        return Some(Vec::new());
    }
    let single: &[&str] = match id {
        FontVariantCaps => &[
            "small-caps",
            "all-small-caps",
            "petite-caps",
            "all-petite-caps",
            "unicase",
            "titling-caps",
        ],
        FontVariantPosition => &["sub", "super"],
        FontVariantLigatures => &["none"],
        _ => &[],
    };
    if single.contains(&v.as_str()) {
        return Some(tags(&v).to_vec());
    }
    let groups = subgroups(id);
    let mut used = vec![false; groups.len()];
    let mut out = Vec::new();
    for word in v.split_whitespace() {
        let g = groups.iter().position(|g| g.contains(&word))?;
        if std::mem::replace(&mut used[g], true) {
            return None;
        }
        out.extend_from_slice(tags(word));
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use LonghandId::*;

    #[test]
    fn combinations_take_one_word_per_subgroup() {
        assert!(variant_tags(FontVariantNumeric, "tabular-nums slashed-zero").is_some());
        assert!(variant_tags(FontVariantNumeric, "tabular-nums proportional-nums").is_none());
        assert!(variant_tags(FontVariantLigatures, "none common-ligatures").is_none());
        assert!(variant_tags(FontVariantCaps, "small-caps unicase").is_none());
        assert!(variant_tags(FontVariantEastAsian, "jis78 ruby full-width").is_some());
        assert_eq!(
            variant_tags(FontVariantPosition, "Normal"),
            Some(Vec::new())
        );
        assert!(variant_tags(FontVariantPosition, "sub super").is_none());
    }
}
