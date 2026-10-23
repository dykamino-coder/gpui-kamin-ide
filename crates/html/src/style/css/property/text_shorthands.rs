//! Сокращения текста, раскрытые при разборе: `white-space` (css-text-4 §3:
//! `white-space-collapse` + `text-wrap-mode`), `text-wrap` (§5.1:
//! `text-wrap-mode` + `text-wrap-style`), `text-decoration`
//! (css-text-decor-4 §2.6) и `text-emphasis` (§3.3). Части идут в любом
//! порядке, слова одной части — подряд (Blink
//! `ConsumeShorthandGreedilyViaLonghands`); опущенная часть — начальное
//! значение.

use super::{LonghandId, ShorthandId};
use LonghandId::*;

pub(super) const WHITE_SPACE: &[LonghandId] = &[WhiteSpaceCollapse, TextWrapMode];
pub(super) const TEXT_WRAP: &[LonghandId] = &[TextWrapMode, TextWrapStyle];
pub(super) const TEXT_DECORATION: &[LonghandId] = &[
    TextDecorationLine,
    TextDecorationThickness,
    TextDecorationStyle,
    TextDecorationColor,
];
pub(super) const TEXT_EMPHASIS: &[LonghandId] = &[TextEmphasisStyle, TextEmphasisColor];

/// Части сокращения: слова, разнесённые по длинным свойствам `kind` (индекс
/// в `longhands` или `None` — слово негодно). Слова одной части идут подряд,
/// часть — не больше одного раза.
pub(super) fn split(
    value: &str,
    longhands: &[LonghandId],
    kind: impl Fn(&str) -> Option<usize>,
) -> Option<Vec<String>> {
    let v = value.trim();
    let mut parts: Vec<Option<String>> = vec![None; longhands.len()];
    let mut current = None;
    for (s, e) in super::font_shorthands::words(v)? {
        let word = &v[s..e];
        let i = kind(&word.to_ascii_lowercase())?;
        match &mut parts[i] {
            Some(text) if current == Some(i) => {
                text.push(' ');
                text.push_str(word);
            }
            Some(_) => return None,
            slot => *slot = Some(word.to_string()),
        }
        current = Some(i);
    }
    if parts.iter().all(Option::is_none) {
        return None;
    }
    Some(
        parts
            .into_iter()
            .zip(longhands)
            .map(|(p, id)| p.unwrap_or_else(|| id.initial().to_string()))
            .collect(),
    )
}

fn collapse_word(w: &str) -> bool {
    matches!(
        w,
        "collapse" | "preserve" | "preserve-breaks" | "break-spaces"
    )
}

/// `white-space: normal | pre | pre-wrap | pre-line | <'white-space-collapse'>
/// || <'text-wrap-mode'>`. Старые слова — пары (§3 table «white-space»):
/// `normal` = collapse wrap, `pre` = preserve nowrap, `pre-wrap` = preserve
/// wrap, `pre-line` = preserve-breaks wrap.
pub(super) fn white_space(value: &str) -> Option<Vec<String>> {
    let pair = |c: &str, m: &str| Some(vec![c.to_string(), m.to_string()]);
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => pair("collapse", "wrap"),
        "pre" => pair("preserve", "nowrap"),
        "pre-wrap" => pair("preserve", "wrap"),
        "pre-line" => pair("preserve-breaks", "wrap"),
        _ => split(value, WHITE_SPACE, |w| match w {
            _ if collapse_word(w) => Some(0),
            "wrap" | "nowrap" => Some(1),
            _ => None,
        }),
    }
}

/// `text-wrap: <'text-wrap-mode'> || <'text-wrap-style'>`.
pub(super) fn text_wrap(value: &str) -> Option<Vec<String>> {
    split(value, TEXT_WRAP, |w| match w {
        "wrap" | "nowrap" => Some(0),
        "auto" | "balance" | "stable" | "pretty" | "avoid-short-last-line" => Some(1),
        _ => None,
    })
}

/// `text-decoration: <line> || <thickness> || <style> || <color>`. Слово
/// толщины — `auto`, `from-font` или длина; прочее, кроме линий и стилей, —
/// цвет (проверяет грамматика своего свойства).
pub(super) fn text_decoration(value: &str) -> Option<Vec<String>> {
    split(value, TEXT_DECORATION, |w| {
        Some(match w {
            "none" | "underline" | "overline" | "line-through" | "blink" | "spelling-error"
            | "grammar-error" => 0,
            "solid" | "double" | "dotted" | "dashed" | "wavy" => 2,
            "auto" | "from-font" | "thin" | "medium" | "thick" => 1,
            _ if w.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '.' | '+' | '-'))
                || ["calc(", "min(", "max(", "clamp("]
                    .iter()
                    .any(|f| w.starts_with(f)) =>
            {
                1
            }
            _ => 3,
        })
    })
}

/// `text-emphasis: <'text-emphasis-style'> || <'text-emphasis-color'>`.
pub(super) fn text_emphasis(value: &str) -> Option<Vec<String>> {
    split(value, TEXT_EMPHASIS, |w| {
        Some(match w {
            "none" | "filled" | "open" | "dot" | "circle" | "double-circle" | "triangle"
            | "sesame" => 0,
            _ if w.starts_with(['"', '\'']) => 0,
            _ => 1,
        })
    })
}

impl ShorthandId {
    /// Длинные свойства сокращений текста.
    pub(super) fn text_longhands(self) -> Option<&'static [LonghandId]> {
        Some(match self {
            Self::WhiteSpace => WHITE_SPACE,
            Self::TextWrap => TEXT_WRAP,
            Self::TextDecoration => TEXT_DECORATION,
            Self::TextEmphasis => TEXT_EMPHASIS,
            _ => return None,
        })
    }
}
