//! Сокращения списков и строк, раскрытые при разборе: `list-style`
//! (css-lists-3 §3.6), `text-align` (css-text-4 §7.1), `text-box`
//! (css-inline-3 §4.1), `line-clamp` и `-webkit-line-clamp` (css-overflow-4
//! §5.2-5.3). Части идут на места `ShorthandId::longhands`; каждую проверяет
//! грамматика её длинного свойства.

use super::{LonghandId, ShorthandId};
use LonghandId::*;

pub(super) const LIST_STYLE: &[LonghandId] = &[ListStylePosition, ListStyleImage, ListStyleType];
pub(super) const TEXT_ALIGN: &[LonghandId] = &[TextAlignAll, TextAlignLast];
pub(super) const TEXT_BOX: &[LonghandId] = &[TextBoxTrim, TextBoxEdge];
pub(super) const LINE_CLAMP: &[LonghandId] = &[MaxLines, BlockEllipsis, Continue];

impl ShorthandId {
    pub(super) fn inline_longhands(self) -> Option<&'static [LonghandId]> {
        Some(match self {
            Self::ListStyle => LIST_STYLE,
            Self::TextAlign => TEXT_ALIGN,
            Self::TextBox => TEXT_BOX,
            Self::LineClamp | Self::WebkitLineClamp => LINE_CLAMP,
            _ => return None,
        })
    }

    pub(super) fn inline_parts(self, value: &str) -> Option<Vec<String>> {
        let v = value.trim();
        let words: Vec<&str> = super::font_shorthands::words(v)?
            .into_iter()
            .map(|(s, e)| &v[s..e])
            .collect();
        match self {
            Self::ListStyle => list_style(&words),
            Self::TextAlign => text_align(&words),
            Self::TextBox => text_box(&words),
            Self::LineClamp => line_clamp(&words),
            Self::WebkitLineClamp => webkit_line_clamp(&words),
            _ => None,
        }
    }
}

fn owned(parts: [&str; 2]) -> Vec<String> {
    parts.iter().map(|p| p.to_string()).collect()
}

/// Слово — `<image>` (по имени функции; грамматику проверит
/// `list-style-image`).
fn is_image(word: &str) -> bool {
    let w = word.to_ascii_lowercase();
    [
        "url(",
        "image(",
        "image-set(",
        "-webkit-image-set(",
        "cross-fade(",
    ]
    .iter()
    .any(|f| w.starts_with(f))
        || w.contains("gradient(")
}

/// `list-style: <'list-style-position'> || <'list-style-image'> ||
/// <'list-style-type'>`. `none` достаётся тем из образа и вида, что не заданы
/// иначе (§3.6); лишний `none` делает значение негодным.
fn list_style(words: &[&str]) -> Option<Vec<String>> {
    let (mut position, mut image, mut kind, mut nones) = (None, None, None, 0);
    for &word in words {
        let lower = word.to_ascii_lowercase();
        if matches!(lower.as_str(), "inside" | "outside") {
            position.replace(word).is_none().then_some(())?;
        } else if lower == "none" {
            nones += 1;
        } else if is_image(word) {
            image.replace(word).is_none().then_some(())?;
        } else {
            kind.replace(word).is_none().then_some(())?;
        }
    }
    if words.is_empty() || nones > usize::from(image.is_none()) + usize::from(kind.is_none()) {
        return None;
    }
    let none = |slot: Option<&str>, initial: &str| {
        slot.unwrap_or(if nones > 0 { "none" } else { initial })
            .to_string()
    };
    Some(vec![
        position.unwrap_or("outside").to_string(),
        none(image, "none"),
        none(kind, "disc"),
    ])
}

/// `text-align: start | end | left | right | center | justify |
/// match-parent | justify-all`: `justify-all` — обоим `justify`,
/// `match-parent` — обоим; прочее — `text-align-all`, а `text-align-last`
/// сбрасывается в `auto` (css-text-4 §7.1).
fn text_align(words: &[&str]) -> Option<Vec<String>> {
    let [word] = words else { return None };
    Some(match word.to_ascii_lowercase().as_str() {
        "justify-all" => owned(["justify", "justify"]),
        "match-parent" => owned(["match-parent", "match-parent"]),
        "auto" => return None,
        _ => owned([word, "auto"]),
    })
}

/// `text-box: normal | <'text-box-trim'> || <'text-box-edge'>`: без обрезки
/// подразумевается `trim-both`, без края — `auto`; `none` не допускается.
fn text_box(words: &[&str]) -> Option<Vec<String>> {
    if let [w] = words
        && w.eq_ignore_ascii_case("normal")
    {
        return Some(owned(["none", "auto"]));
    }
    let is_trim = |w: &str| {
        matches!(
            w.to_ascii_lowercase().as_str(),
            "trim-start" | "trim-end" | "trim-both"
        )
    };
    if words.is_empty() || words.iter().any(|w| w.eq_ignore_ascii_case("none")) {
        return None;
    }
    let (trim, edge) = match words.iter().position(|w| is_trim(w)) {
        None => ("trim-both", words),
        // Обрезка — целиком до или после края (`||` не разрывает край).
        Some(0) => (words[0], &words[1..]),
        Some(i) if i == words.len() - 1 => (words[i], &words[..i]),
        Some(_) => return None,
    };
    let edge = if edge.is_empty() {
        "auto".to_string()
    } else {
        edge.join(" ")
    };
    Some(vec![trim.to_string(), edge])
}

/// `line-clamp: none | [<integer [1,∞]> || auto || <'block-ellipsis'>]
/// -webkit-legacy?` (css-overflow-4 §5.2, редакция WPT
/// `line-clamp-valid` и Blink `AlternativeLineClampShorthand`): `none` —
/// `max-lines: auto`, `no-ellipsis`, `continue: normal`; иначе опущенные
/// предел и `auto` — `auto`, опущенный знак — `ellipsis`, `continue` —
/// `collapse` или `-webkit-legacy`.
fn line_clamp(words: &[&str]) -> Option<Vec<String>> {
    if let [w] = words
        && w.eq_ignore_ascii_case("none")
    {
        return Some(vec!["auto".into(), "no-ellipsis".into(), "normal".into()]);
    }
    let (words, legacy) = match words.split_last() {
        Some((last, rest)) if last.eq_ignore_ascii_case("-webkit-legacy") => (rest, true),
        _ => (words, false),
    };
    let (mut lines, mut auto, mut ellipsis) = (None, false, None);
    for &word in words {
        if word.eq_ignore_ascii_case("auto") && !auto {
            auto = true;
        } else if word.bytes().all(|b| b.is_ascii_digit()) {
            lines.replace(word).is_none().then_some(())?;
        } else {
            ellipsis.replace(word).is_none().then_some(())?;
        }
    }
    if lines.is_none() && !auto && ellipsis.is_none() {
        return None;
    }
    let max_lines = match (lines, auto) {
        (Some(n), true) => format!("{n} auto"),
        (Some(n), false) => n.to_string(),
        (None, _) => "auto".into(),
    };
    let cont = if legacy { "-webkit-legacy" } else { "collapse" };
    Some(vec![
        max_lines,
        ellipsis.unwrap_or("ellipsis").to_string(),
        cont.to_string(),
    ])
}

/// `-webkit-line-clamp: none | <integer [1,∞]>` (css-overflow-4 §5.3): знак
/// всегда `ellipsis`; число — `continue: -webkit-legacy`.
fn webkit_line_clamp(words: &[&str]) -> Option<Vec<String>> {
    let [word] = words else { return None };
    let (lines, cont) = if word.eq_ignore_ascii_case("none") {
        ("auto", "normal")
    } else {
        (*word, "-webkit-legacy")
    };
    Some(vec![lines.to_string(), "ellipsis".into(), cont.into()])
}
