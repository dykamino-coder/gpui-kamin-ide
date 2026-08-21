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

/// Аддитивная система (css-counter-styles-3 §additive): значение
/// набирается из наибольших подходящих знаков подряд.
fn additive(mut n: usize, table: &[(usize, char)]) -> String {
    let mut out = String::new();
    for (weight, sign) in table {
        while n >= *weight {
            out.push(*sign);
            n -= weight;
        }
    }
    out
}

/// Армянская запись, 1..9999 (css-counter-styles-3 §armenian).
const ARMENIAN: &[(usize, char)] = &[
    (9000, 'Ք'), (8000, 'Փ'), (7000, 'Ւ'), (6000, 'Ց'), (5000, 'Ր'), (4000, 'Տ'), (3000, 'Վ'),
    (2000, 'Ս'), (1000, 'Ռ'), (900, 'Ջ'), (800, 'Պ'), (700, 'Չ'), (600, 'Ո'), (500, 'Շ'),
    (400, 'Ն'), (300, 'Յ'), (200, 'Մ'), (100, 'Ճ'), (90, 'Ղ'), (80, 'Ձ'), (70, 'Հ'), (60, 'Կ'),
    (50, 'Ծ'), (40, 'Խ'), (30, 'Լ'), (20, 'Ի'), (10, 'Ժ'), (9, 'Թ'), (8, 'Ը'), (7, 'Է'),
    (6, 'Զ'), (5, 'Ե'), (4, 'Դ'), (3, 'Գ'), (2, 'Բ'), (1, 'Ա'),
];

/// Грузинская запись, 1..19999 (css-counter-styles-3 §georgian).
const GEORGIAN: &[(usize, char)] = &[
    (10000, 'ჵ'), (9000, 'ჰ'), (8000, 'ჯ'), (7000, 'ჴ'), (6000, 'ხ'), (5000, 'ჭ'), (4000, 'წ'),
    (3000, 'ძ'), (2000, 'ც'), (1000, 'ჩ'), (900, 'შ'), (800, 'ყ'), (700, 'ღ'), (600, 'ქ'),
    (500, 'ფ'), (400, 'ჳ'), (300, 'ტ'), (200, 'ს'), (100, 'რ'), (90, 'ჟ'), (80, 'პ'), (70, 'ო'),
    (60, 'ჲ'), (50, 'ნ'), (40, 'მ'), (30, 'ლ'), (20, 'კ'), (10, 'ი'), (9, 'თ'), (8, 'ჱ'),
    (7, 'ზ'), (6, 'ვ'), (5, 'ე'), (4, 'დ'), (3, 'გ'), (2, 'ბ'), (1, 'ა'),
];

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
        // Диапазон стиля — часть его определения: вне его берётся
        // десятичный резерв (css-counter-styles-3 §counter-style-range).
        "armenian" | "upper-armenian" => positive
            .filter(|n| *n <= 9999)
            .map_or_else(|| value.to_string(), |n| additive(n, ARMENIAN)),
        "georgian" => positive
            .filter(|n| *n <= 19999)
            .map_or_else(|| value.to_string(), |n| additive(n, GEORGIAN)),
        _ => value.to_string(),
    }
}

/// Суффикс маркера списка (css-counter-styles-3 §7 `suffix`): у точечных
/// стилей — пробел, у остальных — точка с пробелом.
pub fn suffix(style: &str) -> &'static str {
    match style {
        "disc" | "circle" | "square" | "disclosure-open" | "disclosure-closed" | "none" => " ",
        _ => ". ",
    }
}

/// Готовая строка маркера: представление и суффикс. У `counter()` суффикса
/// нет по спеке, поэтому он живёт отдельной функцией, а не в `repr`.
pub fn marker_repr(value: i32, style: &str) -> String {
    if style == "none" {
        return String::new();
    }
    let body = repr(value, style);
    format!("{body}{}", suffix(style))
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
        assert_eq!(repr(1, "armenian"), "Ա");
        assert_eq!(repr(9999, "armenian"), "ՔՋՂԹ");
        assert_eq!(repr(10000, "armenian"), "10000", "вне диапазона — десятичный");
        assert_eq!(repr(1, "georgian"), "ა");
        assert_eq!(repr(19999, "georgian"), "ჵჰშჟთ");
        assert_eq!(repr(20000, "georgian"), "20000");
        // Незнакомое имя ведёт себя как decimal.
        assert_eq!(repr(5, "cjk-ideographic"), "5");
        assert_eq!(repr(5, "none"), "");
    }
}
