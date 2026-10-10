//! Разбор шрифта: единицы шрифта в длинах, части сокращения font, родовые семейства, font-feature-settings, имена семейств, font-stretch.

use super::*;

/// Есть ли в записи длины в единицах шрифта (`em`, `rem`, `ex`, `ch`).
pub(in crate::style::computed) fn has_font_units(v: &str) -> bool {
    v.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
        .any(|t| {
            let unit = t.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-');
            unit.len() < t.len()
                && matches!(
                    unit.to_ascii_lowercase().as_str(),
                    "em" | "rem" | "ex" | "ch"
                )
        })
}

/// Заменить длины в единицах шрифта на пиксели: `1em` → `16px`.
pub(crate) fn font_lengths_to_px(v: &str, em: f32, rem: f32, ex: f32, ch: f32) -> String {
    let mut out = String::with_capacity(v.len() + 8);
    let mut token = String::new();
    let flush = |token: &mut String, out: &mut String| {
        if token.is_empty() {
            return;
        }
        let unit_at = token
            .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
            .unwrap_or(token.len());
        let (num, unit) = token.split_at(unit_at);
        let k = match unit.to_ascii_lowercase().as_str() {
            "em" => Some(em),
            "rem" => Some(rem),
            "ex" => Some(ex),
            "ch" => Some(ch),
            _ => None,
        };
        match (k, num.parse::<f32>()) {
            (Some(k), Ok(n)) if unit_at > 0 => out.push_str(&format!("{}px", n * k)),
            _ => out.push_str(token),
        }
        token.clear();
    };
    for c in v.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
            token.push(c);
        } else {
            flush(&mut token, &mut out);
            out.push(c);
        }
    }
    flush(&mut token, &mut out);
    out
}

pub(in crate::style::computed) fn split_font(v: &str) -> (&str, &str) {
    let mut end = 0;
    for token in split_outside_parens(v) {
        let at = v[end..]
            .find(token.as_str())
            .map(|i| end + i)
            .unwrap_or(end);
        end = at + token.len();
        if font_size_token(&token) {
            return (&v[..end], v[end..].trim());
        }
    }
    (v, "")
}

/// Кусок головы сокращения `font`, который задаёт КЕГЛЬ (быть может, с
/// `/высотой строки`). Голое число — ВЕС, а не кегль (§15.6: 100…900):
/// `Len::parse("900")` даёт точки, и `font: 900 2em Ahem` отдавал кегль
/// весу, а `2em Ahem` — семейству, после чего гибло всё
/// (`font-family-011`). Ключевые кегли и математические функции — тоже
/// кегль (§15.8; `font: calc(10 * 10px) sans-serif`, `font-148`).
pub(in crate::style::computed) fn font_size_token(token: &str) -> bool {
    let size = font_slash(token).map_or(token, |(s, _)| s);
    let lower = size.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "xx-small"
            | "x-small"
            | "small"
            | "medium"
            | "large"
            | "x-large"
            | "xx-large"
            | "xxx-large"
            | "larger"
            | "smaller"
    ) || ["calc(", "min(", "max(", "clamp("]
        .iter()
        .any(|f| lower.starts_with(f))
    {
        return true;
    }
    size.starts_with(|c: char| c.is_ascii_digit() || c == '.')
        && (token.contains('/')
            || size == "0"
            || (Len::parse(size).is_some() && !size.chars().all(|c| c.is_ascii_digit())))
}

/// Косая черта ВНЕ скобок: `20px/1.5` делится, `calc(100px/2)` — нет.
pub(in crate::style::computed) fn font_slash(t: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (i, ch) in t.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '/' if depth == 0 => return Some((&t[..i], &t[i + 1..])),
            _ => {}
        }
    }
    None
}

/// Семейство, которое подставляется за родовое имя.
///
/// Родовое имя — не шрифт, а разряд: в системе шрифтов его нет, и поиск по
/// нему кончается ничем (а в отладочной сборке — паникой). Подставляются те
/// же семейства, что берёт Chrome на Windows, иначе разметка набирается
/// умолчанием движка и шире эталона.
///
/// `monospace` в таблицу не входит: за него отвечает признак `monospace`, и
/// семейство под него выбирается позже — из тех, что на машине есть
/// (см. `metrics::mono_family`). Здесь он только не считается именем шрифта.
pub fn generic_family(lower: &str) -> Option<&'static str> {
    match lower {
        "system-ui" | "sans-serif" | "ui-sans-serif" | "ui-rounded" => Some(GENERIC_SANS),
        "serif" | "ui-serif" => Some("Times New Roman"),
        "cursive" => Some("Comic Sans MS"),
        "fantasy" => Some("Impact"),
        _ => None,
    }
}

/// Родовое имя семейства — разряд шрифта, а не шрифт.
pub(crate) fn is_generic(lower: &str) -> bool {
    generic_family(lower).is_some() || matches!(lower, "monospace" | "ui-monospace")
}

/// Семейство за родовое `sans-serif`; оно же — умолчание документа.
pub const GENERIC_SANS: &str = "Segoe UI";

/// Список `font-feature-settings` (css-fonts-4 §7.1): `normal` — пустой,
/// иначе `<opentype-tag> [ <integer [0,∞]> | on | off ]?` через запятую.
/// Тег — СТРОКА ровно из четырёх печатных ASCII-знаков в ЛЮБЫХ кавычках:
/// `'liga' off` прежде терялся целиком (`trim_matches('"')` оставлял шесть
/// знаков — `font-feature-resolution-001/002`). Тот же разбор нужен
/// дескриптору в `@font-face`.
pub(crate) fn feature_list(v: &str) -> Option<Vec<(String, u32)>> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("normal") {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for token in v.split(',') {
        let token = token.trim();
        let q = token.chars().next()?;
        if q != '"' && q != '\'' {
            return None;
        }
        let rest = &token[1..];
        let end = rest.find(q)?;
        let tag = &rest[..end];
        if tag.len() != 4 || !tag.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
            return None;
        }
        let on = match rest[end + 1..].trim() {
            "" | "on" => 1,
            "off" => 0,
            n => n.parse::<u32>().ok()?,
        };
        out.push((tag.to_string(), on));
    }
    Some(out)
}

/// Годное имя семейства: строка в кавычках либо ряд идентификаторов.
///
/// Идентификатор по §4.1.3 начинается с буквы, подчёркивания, не-ASCII знака
/// или экранирования; за ними идут буквы, цифры, дефисы, подчёркивания и
/// экранирования. Цифра первой запрещена, дефис с цифрой следом — тоже.
pub(crate) fn family_name_ok(part: &str) -> bool {
    if part.is_empty() {
        return false;
    }
    if (part.starts_with('"') && part.ends_with('"') && part.len() >= 2)
        || (part.starts_with('\'') && part.ends_with('\'') && part.len() >= 2)
    {
        return true;
    }
    part.split_whitespace().all(|word| {
        let mut chars = word.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        let head_ok = first.is_alphabetic()
            || first == '_'
            || first == '\\'
            || first as u32 >= 0xa0
            || (first == '-'
                && word
                    .chars()
                    .nth(1)
                    .is_some_and(|c| c.is_alphabetic() || c == '_' || c as u32 >= 0xa0));
        head_ok
            && word.chars().all(|c| {
                c.is_alphanumeric() || c == '-' || c == '_' || c == '\\' || c as u32 >= 0xa0
            })
    })
}

/// `stretch` and its prefixed spellings (css-sizing-4 §4.1).
pub(in crate::style::computed) fn stretch_keyword(v: &str) -> bool {
    let v = v.trim();
    v.eq_ignore_ascii_case("stretch")
        || v.eq_ignore_ascii_case("-webkit-fill-available")
        || v.eq_ignore_ascii_case("-moz-available")
}
