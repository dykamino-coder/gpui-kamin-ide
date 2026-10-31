//! Встроенные стили счётчика: распознавание имени, описание стиля и его начальное значение.

use super::*;

/// Описание предопределённого стиля как набора дескрипторов `@counter-style`
/// (css-counter-styles-3 §6–§7): его наследует `system: extends <имя>`.
pub(in crate::style::generated) struct Builtin {
    pub negative: (String, String),
    pub suffix: String,
    /// Явный `range` стиля; `None` — по системе (здесь — неограниченный).
    pub range: Option<(i64, i64)>,
    pub pad: Option<(usize, String)>,
    pub fallback: &'static str,
    /// Системы, пишущие знак минуса (§negative: symbolic, alphabetic,
    /// numeric, additive и длинные восточноазиатские).
    pub uses_negative: bool,
}

/// Имена предопределённых стилей. Регистр у них снимается при разборе
/// (§counter-style-name: «the names defined in this specification are ASCII
/// lowercased on parse»), у прочих имён он значим.
fn is_predefined(name: &str) -> bool {
    matches!(
        name,
        "decimal"
            | "decimal-leading-zero"
            | "disc"
            | "circle"
            | "square"
            | "disclosure-open"
            | "disclosure-closed"
            | "lower-roman"
            | "upper-roman"
            | "lower-alpha"
            | "lower-latin"
            | "upper-alpha"
            | "upper-latin"
            | "lower-greek"
            | "cjk-decimal"
            | "hiragana"
            | "hiragana-iroha"
            | "katakana"
            | "katakana-iroha"
            | "cjk-earthly-branch"
            | "cjk-heavenly-stem"
            | "lower-armenian"
            | "armenian"
            | "upper-armenian"
            | "georgian"
            | "hebrew"
            | "ethiopic-numeric"
            | "cjk-ideographic"
    ) || NUMERIC_ZERO.iter().any(|(k, _)| *k == name)
        || CJK_STYLES.iter().any(|(k, _)| *k == name)
}

/// Имя стиля, как его видит каскад: предопределённое — строчными.
pub(in crate::style::generated) fn normalize_name(name: &str) -> std::borrow::Cow<'_, str> {
    if name.bytes().any(|b| b.is_ascii_uppercase()) {
        let low = name.to_ascii_lowercase();
        if is_predefined(&low) {
            return std::borrow::Cow::Owned(low);
        }
    }
    std::borrow::Cow::Borrowed(name)
}

pub(in crate::style::generated) fn builtin(name: &str) -> Option<Builtin> {
    if !is_predefined(name) {
        return None;
    }
    let cjk = match name {
        "cjk-ideographic" => Some(&CJK_STYLES[7].1),
        _ => CJK_STYLES.iter().find(|(k, _)| *k == name).map(|(_, t)| t),
    };
    let negative = cjk.map_or("-", |t| t.negative).to_string();
    let range = match name {
        "lower-roman" | "upper-roman" => Some((1, 3999)),
        "lower-armenian" | "armenian" | "upper-armenian" => Some((1, 9999)),
        "georgian" => Some((1, 19999)),
        "hebrew" => Some((1, 10999)),
        "cjk-decimal" => Some((0, i64::MAX)),
        "lower-alpha" | "lower-latin" | "upper-alpha" | "upper-latin" | "lower-greek"
        | "hiragana" | "hiragana-iroha" | "katakana" | "katakana-iroha" | "ethiopic-numeric" => {
            Some((1, i64::MAX))
        }
        _ if cjk.is_some() => Some((-9_999_999_999_999_999, 9_999_999_999_999_999)),
        _ => None,
    };
    let fallback = match name {
        "cjk-earthly-branch" | "cjk-heavenly-stem" => "cjk-decimal",
        _ if cjk.is_some() => "cjk-decimal",
        _ => "decimal",
    };
    let uses_negative = !matches!(
        name,
        "disc"
            | "circle"
            | "square"
            | "disclosure-open"
            | "disclosure-closed"
            | "cjk-earthly-branch"
            | "cjk-heavenly-stem"
    );
    Some(Builtin {
        negative: (negative, String::new()),
        suffix: suffix(name).to_string(),
        range,
        pad: (name == "decimal-leading-zero").then(|| (2, "0".to_string())),
        fallback,
        uses_negative,
    })
}

/// Начальное представление `n >= 0` предопределённым стилем — без знака,
/// без дополнения и без проверки диапазона (их решает общий алгоритм
/// §counter-style-generate в `counter_style_rules`). `None` — система стиля
/// значение не представляет (аддитивный ноль, фиксированный вне набора).
pub(in crate::style::generated) fn builtin_initial(name: &str, n: u64) -> Option<String> {
    let positive = usize::try_from(n).ok().filter(|n| *n > 0);
    // CSS permits a fallback above 60 code points; bound repeated top weights.
    let top = match name {
        "lower-roman" | "upper-roman" => Some(1000),
        "lower-armenian" | "armenian" | "upper-armenian" => Some(9000),
        "georgian" | "hebrew" => Some(10000),
        _ => None,
    };
    if top.is_some_and(|weight| n / weight > 60) {
        return None;
    }
    Some(match name {
        "decimal" | "decimal-leading-zero" => n.to_string(),
        "disc" => "•".to_string(),
        "circle" => "◦".to_string(),
        "square" => "▪".to_string(),
        "disclosure-open" => "▾".to_string(),
        "disclosure-closed" => "▸".to_string(),
        "lower-roman" => roman(positive?),
        "upper-roman" => roman(positive?).to_uppercase(),
        "lower-alpha" | "lower-latin" => alphabetic(positive?, b'a'),
        "upper-alpha" | "upper-latin" => alphabetic(positive?, b'A'),
        "lower-greek" => lower_greek(positive?),
        "cjk-decimal" => numeric_unsigned(n, &CJK_DIGITS),
        "hiragana" | "hiragana-iroha" | "katakana" | "katakana-iroha" => {
            let letters: Vec<char> = match name {
                "hiragana" => HIRAGANA,
                "hiragana-iroha" => HIRAGANA_IROHA,
                "katakana" => KATAKANA,
                _ => KATAKANA_IROHA,
            }
            .chars()
            .collect();
            alphabetic_of(positive?, &letters)
        }
        "cjk-earthly-branch" | "cjk-heavenly-stem" => {
            let letters: Vec<char> = if name == "cjk-earthly-branch" {
                EARTHLY_BRANCH
            } else {
                HEAVENLY_STEM
            }
            .chars()
            .collect();
            letters.get(positive? - 1)?.to_string()
        }
        "lower-armenian" => additive(positive?, ARMENIAN).to_lowercase(),
        "armenian" | "upper-armenian" => additive(positive?, ARMENIAN),
        "georgian" => additive(positive?, GEORGIAN),
        "hebrew" => additive_str(positive?, HEBREW),
        "ethiopic-numeric" => ethiopic(positive?),
        name => {
            if let Some((_, zero)) = NUMERIC_ZERO.iter().find(|(k, _)| *k == name) {
                let digits: Vec<char> = (0..10)
                    .filter_map(|d| char::from_u32(*zero as u32 + d))
                    .collect();
                numeric_unsigned(n, &digits)
            } else {
                let t = if name == "cjk-ideographic" {
                    &CJK_STYLES[7].1
                } else {
                    &CJK_STYLES.iter().find(|(k, _)| *k == name)?.1
                };
                if n > 9_999_999_999_999_999 {
                    return None;
                }
                cjk_positional(n as usize, t)
            }
        }
    })
}
