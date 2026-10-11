//! Разбор фона: функция картинки, валидность сокращения background, список слоёв через запятую, пары процент/точки.

use super::*;

/// Функция картинки в начале слоя и хвост за её закрывающей скобкой.
pub(in crate::style::computed) fn split_image_func(v: &str) -> (&str, &str) {
    let mut depth = 0i32;
    for (i, ch) in v.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return (&v[..=i], &v[i + 1..]);
                }
            }
            _ => {}
        }
    }
    (v, "")
}

/// Одна грань размещения: `3`, `span 2`, `auto`.
/// Годно ли значение сокращения `background` целиком.
///
/// Негодное объявление ОТБРАСЫВАЕТСЯ, а не сбрасывает свои длинные свойства
/// (CSS 2.1 §4.1.7): `background: green` в одном правиле и `background: red\;`
/// (значение с экранированной точкой с запятой, то есть цвет `red;`) в
/// другом обязаны оставить фон зелёным. Разбор ниже намеренно снисходителен —
/// он берёт из записи всё, что узнал, — поэтому годность проверяется
/// отдельно, и только ею решается сброс (`escapes-002/014`, `keywords-000`).
pub(in crate::style::computed) fn background_shorthand_valid(v: &str) -> bool {
    if v.contains("gradient(") || v.contains("url(") {
        return true;
    }
    let mut any = false;
    for token in split_outside_parens(v) {
        // Запятая слоя (`none, none`) — не часть слова.
        let t = token.trim().trim_end_matches(',').trim();
        if t.is_empty() || t == "/" {
            continue;
        }
        any = true;
        let known = matches!(
            t,
            "none"
                | "transparent"
                | "initial"
                | "unset"
                | "revert"
                | "no-repeat"
                | "repeat"
                | "repeat-x"
                | "repeat-y"
                | "space"
                | "round"
                | "cover"
                | "contain"
                | "scroll"
                | "fixed"
                | "local"
                | "border-box"
                | "padding-box"
                | "content-box"
                | "text"
                | "left"
                | "right"
                | "top"
                | "bottom"
                | "center"
        ) || Len::parse(t).is_some()
            || Color::parse(t).is_some()
            || parse_image_color(t).is_some();
        if !known {
            return false;
        }
    }
    any
}

/// Фоновые свойства со списком слоёв (css-backgrounds-3 §2.1).
pub(crate) const BG_LIST_KEYS: [&str; 8] = [
    "background",
    "background-image",
    "background-size",
    "background-position",
    "background-repeat",
    "background-origin",
    "background-clip",
    "background-attachment",
];

/// Слои фона: значение режется по запятым ВНЕ скобок.
///
/// Запятая внутри `rgba(…)` или `linear-gradient(…)` слой не кончает, поэтому
/// делить строку простым `split(',')` нельзя.
pub(crate) fn background_layers(v: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut rest = v;
    while let Some(at) = top_level_comma(rest) {
        out.push(rest[..at].trim());
        rest = &rest[at + 1..];
    }
    out.push(rest.trim());
    out
}

pub(crate) fn top_level_comma(inner: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Процентная смесь `calc(A% ± Bpx)` парой (доля, точки) — для свойств,
/// которые доли решают САМИ при отрисовке, зная размер коробки
/// (css-values-4 §10.9). Любая другая природа в сумме (`ch`, `vw`, `em`…) —
/// `None`: её здесь сложить не с чем, и запись, как прежде, не применяется.
pub(in crate::style::computed) fn pct_px_pair(t: &str) -> Option<(f32, f32)> {
    match Len::parse_mixed(t)? {
        Len::Calc(i) => crate::style::values::value::calc_get(i).pct_px(),
        _ => None,
    }
}
