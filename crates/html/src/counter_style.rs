//! Представление счётчика знаками: `counter(n, lower-roman)`, маркеры
//! списков, `counters()`.
//!
//! Одна таблица на весь крейт: маркеры списка и генерируемое содержимое
//! обязаны совпадать знак в знак — раньше римские цифры жили отдельной
//! копией в отрисовке списков, и `content: counter(c, lower-roman)`
//! получить их не мог вовсе.

/// Римская запись строчными; ноль и отрицательное римскими не пишутся
/// (css-counter-styles §additive: вне диапазона — десятичный резерв).
pub fn roman(mut n: usize) -> String {
    const TABLE: &[(usize, &str)] = &[
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut out = String::new();
    for (value, sign) in TABLE {
        while n >= *value {
            out.push_str(sign);
            n -= value;
        }
    }
    out
}

/// Алфавитная запись: 1 → a, 26 → z, 27 → aa (css-counter-styles §alphabetic).
fn alphabetic(mut n: usize, base: u8) -> String {
    let mut out = vec![];
    while n > 0 {
        let rem = (n - 1) % 26;
        out.push((base + rem as u8) as char);
        n = (n - 1) / 26;
    }
    out.iter().rev().collect()
}

/// Греческая строчная: α, β, γ… (без ς — css-counter-styles §lower-greek).
fn lower_greek(n: usize) -> String {
    const LETTERS: &[char] = &[
        'α', 'β', 'γ', 'δ', 'ε', 'ζ', 'η', 'θ', 'ι', 'κ', 'λ', 'μ', 'ν', 'ξ', 'ο', 'π', 'ρ', 'σ',
        'τ', 'υ', 'φ', 'χ', 'ψ', 'ω',
    ];
    let mut out = vec![];
    let mut n = n;
    while n > 0 {
        let rem = (n - 1) % LETTERS.len();
        out.push(LETTERS[rem]);
        n = (n - 1) / LETTERS.len();
    }
    out.iter().rev().collect()
}

/// Значение счётчика знаками названного стиля.
///
/// Незнакомый стиль — десятичный (css-counter-styles §counter-style-name:
/// неизвестное имя ведёт себя как `decimal`). Стили с конечным диапазоном
/// (римский, алфавитные) вне его тоже падают на десятичный.
pub fn repr(value: i32, style: &str) -> String {
    let positive = usize::try_from(value).ok().filter(|n| *n > 0);
    match style {
        "none" => String::new(),
        "disc" => "•".to_string(),
        "circle" => "◦".to_string(),
        "square" => "▪".to_string(),
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
        "lower-roman" => positive.map_or_else(|| value.to_string(), roman),
        "upper-roman" => positive.map_or_else(|| value.to_string(), |n| roman(n).to_uppercase()),
        "lower-alpha" | "lower-latin" => {
            positive.map_or_else(|| value.to_string(), |n| alphabetic(n, b'a'))
        }
        "upper-alpha" | "upper-latin" => {
            positive.map_or_else(|| value.to_string(), |n| alphabetic(n, b'A'))
        }
        "lower-greek" => positive.map_or_else(|| value.to_string(), lower_greek),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_follow_spec() {
        assert_eq!(repr(4, "lower-roman"), "iv");
        assert_eq!(repr(4, "upper-roman"), "IV");
        assert_eq!(repr(27, "lower-alpha"), "aa");
        assert_eq!(repr(1, "upper-latin"), "A");
        assert_eq!(repr(3, "lower-greek"), "γ");
        assert_eq!(repr(7, "decimal-leading-zero"), "07");
        assert_eq!(repr(-7, "decimal-leading-zero"), "-07");
        assert_eq!(repr(12, "decimal-leading-zero"), "12");
        // Вне диапазона стиля — десятичный резерв.
        assert_eq!(repr(0, "lower-roman"), "0");
        assert_eq!(repr(-3, "upper-alpha"), "-3");
        // Незнакомое имя ведёт себя как decimal.
        assert_eq!(repr(5, "cjk-ideographic"), "5");
        assert_eq!(repr(5, "none"), "");
    }
}
