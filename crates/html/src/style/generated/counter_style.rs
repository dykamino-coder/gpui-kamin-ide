//! CSS Counter Styles 3: shared representations for counters and list markers.
//! CJK styles implement the extended range of section 7.1.5.

mod additive_tables;
mod builtin;
mod cjk;
mod cjk_data;
mod numeric;
use additive_tables::{ARMENIAN, GEORGIAN, HEBREW, ethiopic};
pub(super) use builtin::{builtin, builtin_initial, normalize_name};
use cjk::{CJK_STYLES, cjk_positional, cjk_repr};
pub use numeric::roman;
use numeric::{
    CJK_DIGITS, EARTHLY_BRANCH, HEAVENLY_STEM, HIRAGANA, HIRAGANA_IROHA, KATAKANA, KATAKANA_IROHA,
    NUMERIC_ZERO, additive, additive_str, alphabetic, alphabetic_of, lower_greek, numeric,
    numeric_unsigned,
};

/// Значение счётчика знаками названного стиля.
///
/// Незнакомый стиль — десятичный (css-counter-styles §counter-style-name:
/// неизвестное имя ведёт себя как `decimal`). Стили с конечным диапазоном
/// (римский, алфавитные) вне его тоже падают на десятичный.
pub fn repr(value: i64, style: &str) -> String {
    let style = &*normalize_name(style);
    if let Some(s) = crate::style::generated::counter_style_rules::custom_repr(value, style) {
        return s;
    }
    builtin_repr(value, style)
}

/// Предопределённый стиль (или десятичный для незнакомого имени) без учёта
/// правил `@counter-style` документа.
pub(super) fn builtin_repr(value: i64, style: &str) -> String {
    let positive = usize::try_from(value).ok().filter(|n| *n > 0);
    match style {
        "none" => String::new(),
        "disc" => "•".to_string(),
        "circle" => "◦".to_string(),
        "square" => "▪".to_string(),
        // css-counter-styles-3 §6.2: `disclosure-open` ▾, `disclosure-closed` ▸.
        "disclosure-open" => "▾".to_string(),
        "disclosure-closed" => "▸".to_string(),
        "decimal-leading-zero" => {
            // Знак остаётся впереди: `-1` — это `-01`.
            let sign = if value < 0 { "-" } else { "" };
            let abs = value.unsigned_abs();
            if abs < 10 {
                format!("{sign}0{abs}")
            } else {
                format!("{sign}{abs}")
            }
        }
        // Римские — аддитивные с диапазоном 1..3999 (css-counter-styles-3
        // §6.1: `range: 1 3999`): 4000 пишется десятичным резервом, а не
        // `mmmm` (`css3-counter-styles-021/025`, `-020b` «straddling range»).
        "lower-roman" => positive
            .filter(|n| *n <= 3999)
            .map_or_else(|| value.to_string(), roman),
        "upper-roman" => positive
            .filter(|n| *n <= 3999)
            .map_or_else(|| value.to_string(), |n| roman(n).to_uppercase()),
        "lower-alpha" | "lower-latin" => {
            positive.map_or_else(|| value.to_string(), |n| alphabetic(n, b'a'))
        }
        "upper-alpha" | "upper-latin" => {
            positive.map_or_else(|| value.to_string(), |n| alphabetic(n, b'A'))
        }
        "lower-greek" => positive.map_or_else(|| value.to_string(), lower_greek),
        // CJK decimal alone has range 0..infinite (CSS Counter Styles 3 section 6.1).
        name if NUMERIC_ZERO.iter().any(|(k, _)| *k == name) => {
            let zero = NUMERIC_ZERO
                .iter()
                .find(|(k, _)| *k == name)
                .map_or('0', |(_, c)| *c) as u32;
            let digits: Vec<char> = (0..10).filter_map(|d| char::from_u32(zero + d)).collect();
            numeric(value, &digits)
        }
        "cjk-decimal" if value >= 0 => numeric(value, &CJK_DIGITS),
        // Кана: алфавитные системы, за концом набора запись удлиняется.
        "hiragana" | "hiragana-iroha" | "katakana" | "katakana-iroha" => {
            let letters: Vec<char> = match style {
                "hiragana" => HIRAGANA,
                "hiragana-iroha" => HIRAGANA_IROHA,
                "katakana" => KATAKANA,
                _ => KATAKANA_IROHA,
            }
            .chars()
            .collect();
            positive.map_or_else(|| value.to_string(), |n| alphabetic_of(n, &letters))
        }
        // Циклы: за пределом цикла резерв НЕ десятичный, а `cjk-decimal`
        // (`css3-counter-styles-202`: 13 -> `一三`, не `13`).
        "cjk-earthly-branch" | "cjk-heavenly-stem" => {
            let letters: Vec<char> = if style == "cjk-earthly-branch" {
                EARTHLY_BRANCH
            } else {
                HEAVENLY_STEM
            }
            .chars()
            .collect();
            positive.filter(|n| *n <= letters.len()).map_or_else(
                || repr(value, "cjk-decimal"),
                |n| letters[n - 1].to_string(),
            )
        }
        // Строчная армянская — тот же аддитивный набор в нижнем регистре.
        "lower-armenian" => positive.filter(|n| *n <= 9999).map_or_else(
            || value.to_string(),
            |n| additive(n, ARMENIAN).to_lowercase(),
        ),
        // Диапазон стиля — часть его определения: вне его берётся
        // десятичный резерв (css-counter-styles-3 §counter-style-range).
        "armenian" | "upper-armenian" => positive
            .filter(|n| *n <= 9999)
            .map_or_else(|| value.to_string(), |n| additive(n, ARMENIAN)),
        "georgian" => positive
            .filter(|n| *n <= 19999)
            .map_or_else(|| value.to_string(), |n| additive(n, GEORGIAN)),
        // CSS Counter Styles 3 defines Hebrew as additive in 1..10999.
        "hebrew" => positive
            .filter(|n| *n <= 10999)
            .map_or_else(|| value.to_string(), |n| additive_str(n, HEBREW)),
        // Эфиопский определён для всех положительных; ноль и отрицательные —
        // десятичным (`counter-ethiopic-numeric`: `0`, `-1`).
        "ethiopic-numeric" => positive.map_or_else(|| value.to_string(), ethiopic),
        // Позиционные восточноазиатские (§6.4). `cjk-ideographic` спека
        // объявляет тождественным `trad-chinese-informal` («It exists for
        // legacy reasons»).
        "cjk-ideographic" => cjk_repr(value, &CJK_STYLES[7].1),
        name => CJK_STYLES
            .iter()
            .find(|(k, _)| *k == name)
            .map_or_else(|| value.to_string(), |(_, t)| cjk_repr(value, t)),
    }
}

/// Суффикс маркера списка (css-counter-styles-3 §7 `suffix`): у точечных
/// стилей — пробел, у остальных — точка с пробелом.
pub fn suffix(style: &str) -> &'static str {
    match style {
        "disc" | "circle" | "square" | "disclosure-open" | "disclosure-closed" | "none" => " ",
        // Восточноазиатские стили ставят идеографическую запятую и БЕЗ
        // пробела (css-counter-styles-3 §6.2/§6.3; сверено с тестами
        // `css3-counter-styles-005/032/035/038/041/203/206`).
        "cjk-decimal"
        | "hiragana"
        | "hiragana-iroha"
        | "katakana"
        | "katakana-iroha"
        | "cjk-earthly-branch"
        | "cjk-heavenly-stem"
        | "japanese-formal"
        | "japanese-informal"
        | "simp-chinese-formal"
        | "simp-chinese-informal"
        | "trad-chinese-formal"
        | "trad-chinese-informal" => "、",
        // Корейские — запятая с пробелом (css-counter-styles-3 §6.3
        // `suffix: ', '`; Blink `ua_counter_style_map.cc`, эталон
        // `counter-suffix-ref`: «일, »).
        "korean-hangul-formal" | "korean-hanja-formal" | "korean-hanja-informal" => ", ",
        // `cjk-ideographic` — тот же `trad-chinese-informal`, значит и суффикс
        // его (§cjk-ideographic).
        "cjk-ideographic" => "、",
        // Эфиопский ставит косую с пробелом (§ethiopic-numeric `suffix: "/ "`,
        // Blink `ua_counter_style_map.cc:424`).
        "ethiopic-numeric" => "/ ",
        _ => ". ",
    }
}

/// Готовая строка маркера: представление и суффикс. У `counter()` суффикса
/// нет по спеке, поэтому он живёт отдельной функцией, а не в `repr`.
pub fn marker_repr(value: i64, style: &str) -> String {
    if style == "none" {
        return String::new();
    }
    let style = &*normalize_name(style);
    if let Some(s) = crate::style::generated::counter_style_rules::custom_marker(value, style) {
        return s;
    }
    let body = repr(value, style);
    format!("{body}{}", suffix(style))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod extended_tests;
