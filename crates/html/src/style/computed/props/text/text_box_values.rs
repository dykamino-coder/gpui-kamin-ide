//! Разбор `text-box-trim`, `text-box-edge` и сокращения `text-box`
//! (css-inline-3 §text-box-trim, §text-box-edge, §text-box) — чистые функции.
//!
//! Грамматика края: `auto | <text-edge>`, где `<text-edge>` —
//! `[text | ideographic | ideographic-ink] |
//! [text | ideographic | ideographic-ink | cap | ex]
//! [text | ideographic | ideographic-ink | alphabetic]`. Одно слово задаёт
//! оба края, если годится для нижнего, иначе нижний — `text` (`cap`, `ex`);
//! `alphabetic` одно — ошибка. Неверное значение объявление отбрасывает
//! (`None`), а не сбрасывает свойство.

use crate::style::computed::TextEdge;

/// `text-box-trim`: (начало, конец).
pub(crate) fn parse_trim(v: &str) -> Option<(bool, bool)> {
    match v {
        "none" => Some((false, false)),
        "trim-start" => Some((true, false)),
        "trim-end" => Some((false, true)),
        "trim-both" => Some((true, true)),
        _ => None,
    }
}

fn shared(w: &str) -> Option<TextEdge> {
    match w {
        "text" => Some(TextEdge::Text),
        "ideographic" => Some(TextEdge::Ideographic),
        "ideographic-ink" => Some(TextEdge::IdeographicInk),
        _ => None,
    }
}

/// `text-box-edge`: (верхний, нижний).
pub(crate) fn parse_edge(v: &str) -> Option<(TextEdge, TextEdge)> {
    let words: Vec<&str> = v.split_ascii_whitespace().collect();
    let over = |w: &str| {
        shared(w).or(match w {
            "cap" => Some(TextEdge::Cap),
            "ex" => Some(TextEdge::Ex),
            _ => None,
        })
    };
    let under = |w: &str| shared(w).or((w == "alphabetic").then_some(TextEdge::Alphabetic));
    match words.as_slice() {
        ["auto"] => Some((TextEdge::Text, TextEdge::Text)),
        [o] => {
            let o = over(o)?;
            Some((o, shared_or_text(o)))
        }
        [o, u] => Some((over(o)?, under(u)?)),
        _ => None,
    }
}

fn shared_or_text(o: TextEdge) -> TextEdge {
    match o {
        TextEdge::Cap | TextEdge::Ex => TextEdge::Text,
        other => other,
    }
}

/// Сокращение `text-box`: `normal | <'text-box-trim'> || <'text-box-edge'>`.
/// Без обрезки подразумевается `trim-both` (не начальное значение), без
/// края — `auto`. Результат: ((начало, конец), (верхний, нижний)).
pub(crate) type TextBox = ((bool, bool), (TextEdge, TextEdge));

pub(crate) fn parse_shorthand(v: &str) -> Option<TextBox> {
    if v == "normal" {
        return Some(((false, false), (TextEdge::Text, TextEdge::Text)));
    }
    let words: Vec<&str> = v.split_ascii_whitespace().collect();
    let trims: Vec<usize> = (0..words.len())
        .filter(|&i| parse_trim(words[i]).is_some())
        .collect();
    let (trim, edge_words): ((bool, bool), Vec<&str>) = match trims.as_slice() {
        [] => ((true, true), words.clone()),
        // Обрезка — целиком до или после края (`||` не разрывает край).
        [i] if *i == 0 || *i == words.len() - 1 => {
            let rest = words
                .iter()
                .enumerate()
                .filter(|(k, _)| k != i)
                .map(|(_, w)| *w)
                .collect();
            (parse_trim(words[*i])?, rest)
        }
        _ => return None,
    };
    // `none` в сокращении не допускается (там `normal`).
    if words.contains(&"none") || words.is_empty() {
        return None;
    }
    let edge = if edge_words.is_empty() {
        (TextEdge::Text, TextEdge::Text)
    } else {
        parse_edge(&edge_words.join(" "))?
    };
    Some((trim, edge))
}

#[cfg(test)]
mod tests {
    use super::*;
    use TextEdge::*;

    #[test]
    fn edge_grammar() {
        assert_eq!(parse_edge("auto"), Some((Text, Text)));
        assert_eq!(parse_edge("cap"), Some((Cap, Text)));
        assert_eq!(parse_edge("ex alphabetic"), Some((Ex, Alphabetic)));
        assert_eq!(parse_edge("ideographic"), Some((Ideographic, Ideographic)));
        assert_eq!(
            parse_edge("text ideographic-ink"),
            Some((Text, IdeographicInk))
        );
        assert_eq!(parse_edge("alphabetic"), None);
        assert_eq!(parse_edge("cap cap"), None);
        assert_eq!(parse_edge("alphabetic text"), None);
        assert_eq!(parse_edge("text text text"), None);
        assert_eq!(parse_edge("auto text"), None);
    }

    #[test]
    fn trim_and_shorthand() {
        assert_eq!(parse_trim("trim-end"), Some((false, true)));
        assert_eq!(parse_trim("both"), None);
        assert_eq!(
            parse_shorthand("normal"),
            Some(((false, false), (Text, Text)))
        );
        assert_eq!(
            parse_shorthand("cap alphabetic"),
            Some(((true, true), (Cap, Alphabetic)))
        );
        assert_eq!(
            parse_shorthand("trim-start text"),
            Some(((true, false), (Text, Text)))
        );
        assert_eq!(
            parse_shorthand("ex trim-end"),
            Some(((false, true), (Ex, Text)))
        );
        assert_eq!(
            parse_shorthand("auto trim-both"),
            Some(((true, true), (Text, Text)))
        );
        assert_eq!(parse_shorthand("none"), None);
        assert_eq!(parse_shorthand("cap trim-start alphabetic"), None);
        assert_eq!(parse_shorthand("trim-start trim-end"), None);
    }
}
