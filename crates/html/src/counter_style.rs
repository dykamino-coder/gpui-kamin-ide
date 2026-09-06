//! ★ ЗАМЕР 05.09 (+0/−3) БЫЛ ЛОЖНЫМ: эталоны семьи `css3-counter-styles-*`
//! рисовались ПУСТЫМИ (не-`li` дети `<ol>` выбрасывались в `render.rs::list`),
//! и таблицы знаков было не с чем сравнивать. После починки эталона (06.09)
//! те же таблицы дали +48/−11 на срезе 501 пары — см. `scout-counterstyles-
//! 2026-09.md`. Оставшиеся 11 — позиционные CJK (шаг 3), они были
//! «зелёными» пустотой.
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

/// Числовая система (css-counter-styles-3 §numeric): запись цифрами своего
/// набора по основанию, равному его длине. Ноль — нулевая цифра, знак минус
/// идёт впереди.
fn numeric(value: i32, digits: &[char]) -> String {
    let base = digits.len();
    let mut n = value.unsigned_abs() as usize;
    if n == 0 {
        return digits[0].to_string();
    }
    let mut out = vec![];
    while n > 0 {
        out.push(digits[n % base]);
        n /= base;
    }
    if value < 0 {
        out.push('-');
    }
    out.iter().rev().collect()
}

/// Алфавитная система по ПРОИЗВОЛЬНОМУ набору знаков
/// (css-counter-styles-3 §alphabetic): 1 — первый знак, len+1 — «первый
/// первый» (`hiragana` на 49 даёт `ああ` — `css3-counter-styles-031`).
fn alphabetic_of(mut n: usize, letters: &[char]) -> String {
    let base = letters.len();
    let mut out = vec![];
    while n > 0 {
        out.push(letters[(n - 1) % base]);
        n = (n - 1) / base;
    }
    out.iter().rev().collect()
}

/// Нулевая цифра «простых числовых» систем (css-counter-styles-3 §6.1).
/// Блоки цифр в Unicode непрерывны, поэтому хранится ОДИН код. Коды сняты с
/// тестов набора (`css3-counter-styles-101…155`).
const NUMERIC_ZERO: &[(&str, char)] = &[
    ("arabic-indic", '\u{0660}'),
    ("bengali", '\u{09E6}'),
    ("cambodian", '\u{17E0}'),
    ("devanagari", '\u{0966}'),
    ("gujarati", '\u{0AE6}'),
    ("gurmukhi", '\u{0A66}'),
    ("kannada", '\u{0CE6}'),
    ("khmer", '\u{17E0}'),
    ("lao", '\u{0ED0}'),
    ("malayalam", '\u{0D66}'),
    ("mongolian", '\u{1810}'),
    ("myanmar", '\u{1040}'),
    ("oriya", '\u{0B66}'),
    ("persian", '\u{06F0}'),
    ("tamil", '\u{0BE6}'),
    ("telugu", '\u{0C66}'),
    ("thai", '\u{0E50}'),
    ("tibetan", '\u{0F20}'),
];

/// `cjk-decimal` — тоже числовая система, но её ноль стоит отдельно от цифр.
const CJK_DIGITS: [char; 10] = ['〇', '一', '二', '三', '四', '五', '六', '七', '八', '九'];

/// Кана и циклы (css-counter-styles-3 §6.2, §6.3). Порядок сверен с тестами
/// `css3-counter-styles-030/033/036/039/201/204`.
const HIRAGANA: &str = "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわゐゑをん";
const HIRAGANA_IROHA: &str = "いろはにほへとちりぬるをわかよたれそつねならむうゐのおくやまけふこえてあさきゆめみしゑひもせす";
const KATAKANA: &str = "アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヰヱヲン";
const KATAKANA_IROHA: &str = "イロハニホヘトチリヌルヲワカヨタレソツネナラムウヰノオクヤマケフコエテアサキユメミシヱヒモセス";
const EARTHLY_BRANCH: &str = "子丑寅卯辰巳午未申酉戌亥";
const HEAVENLY_STEM: &str = "甲乙丙丁戊己庚辛壬癸";

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
    (9000, 'Ք'),
    (8000, 'Փ'),
    (7000, 'Ւ'),
    (6000, 'Ց'),
    (5000, 'Ր'),
    (4000, 'Տ'),
    (3000, 'Վ'),
    (2000, 'Ս'),
    (1000, 'Ռ'),
    (900, 'Ջ'),
    (800, 'Պ'),
    (700, 'Չ'),
    (600, 'Ո'),
    (500, 'Շ'),
    (400, 'Ն'),
    (300, 'Յ'),
    (200, 'Մ'),
    (100, 'Ճ'),
    (90, 'Ղ'),
    (80, 'Ձ'),
    (70, 'Հ'),
    (60, 'Կ'),
    (50, 'Ծ'),
    (40, 'Խ'),
    (30, 'Լ'),
    (20, 'Ի'),
    (10, 'Ժ'),
    (9, 'Թ'),
    (8, 'Ը'),
    (7, 'Է'),
    (6, 'Զ'),
    (5, 'Ե'),
    (4, 'Դ'),
    (3, 'Գ'),
    (2, 'Բ'),
    (1, 'Ա'),
];

/// Грузинская запись, 1..19999 (css-counter-styles-3 §georgian).
const GEORGIAN: &[(usize, char)] = &[
    (10000, 'ჵ'),
    (9000, 'ჰ'),
    (8000, 'ჯ'),
    (7000, 'ჴ'),
    (6000, 'ხ'),
    (5000, 'ჭ'),
    (4000, 'წ'),
    (3000, 'ძ'),
    (2000, 'ც'),
    (1000, 'ჩ'),
    (900, 'შ'),
    (800, 'ყ'),
    (700, 'ღ'),
    (600, 'ქ'),
    (500, 'ფ'),
    (400, 'ჳ'),
    (300, 'ტ'),
    (200, 'ს'),
    (100, 'რ'),
    (90, 'ჟ'),
    (80, 'პ'),
    (70, 'ო'),
    (60, 'ჲ'),
    (50, 'ნ'),
    (40, 'მ'),
    (30, 'ლ'),
    (20, 'კ'),
    (10, 'ი'),
    (9, 'თ'),
    (8, 'ჱ'),
    (7, 'ზ'),
    (6, 'ვ'),
    (5, 'ე'),
    (4, 'დ'),
    (3, 'გ'),
    (2, 'ბ'),
    (1, 'ა'),
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
        // Простые числовые: цифры своего набора, основание десять. Ноль и
        // отрицательные пишутся так же, как десятичным (§numeric), поэтому
        // фильтра диапазона здесь нет.
        name if NUMERIC_ZERO.iter().any(|(k, _)| *k == name) => {
            let zero = NUMERIC_ZERO
                .iter()
                .find(|(k, _)| *k == name)
                .map_or('0', |(_, c)| *c) as u32;
            let digits: Vec<char> = (0..10).filter_map(|d| char::from_u32(zero + d)).collect();
            numeric(value, &digits)
        }
        "cjk-decimal" => numeric(value, &CJK_DIGITS),
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
            positive
                .filter(|n| *n <= letters.len())
                .map_or_else(|| numeric(value, &CJK_DIGITS), |n| letters[n - 1].to_string())
        }
        // Строчная армянская — тот же аддитивный набор в нижнем регистре.
        "lower-armenian" => positive
            .filter(|n| *n <= 9999)
            .map_or_else(|| value.to_string(), |n| additive(n, ARMENIAN).to_lowercase()),
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
        // Восточноазиатские стили ставят идеографическую запятую и БЕЗ
        // пробела (css-counter-styles-3 §6.2/§6.3; сверено с тестами
        // `css3-counter-styles-005/032/035/038/041/203/206`).
        "cjk-decimal" | "hiragana" | "hiragana-iroha" | "katakana" | "katakana-iroha"
        | "cjk-earthly-branch" | "cjk-heavenly-stem" | "japanese-formal"
        | "japanese-informal" | "simp-chinese-formal" | "simp-chinese-informal"
        | "trad-chinese-formal" | "trad-chinese-informal" => "、",
        // Корейские — запятая с пробелом (css-counter-styles-3 §6.3
        // `suffix: ', '`; Blink `ua_counter_style_map.cc`, эталон
        // `counter-suffix-ref`: «일, »).
        "korean-hangul-formal" | "korean-hanja-formal" | "korean-hanja-informal" => ", ",
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
        assert_eq!(
            repr(10000, "armenian"),
            "10000",
            "вне диапазона — десятичный"
        );
        assert_eq!(repr(1, "georgian"), "ა");
        assert_eq!(repr(19999, "georgian"), "ჵჰშჟთ");
        assert_eq!(repr(20000, "georgian"), "20000");
        // Незнакомое имя ведёт себя как decimal.
        assert_eq!(repr(5, "cjk-ideographic"), "5");
        assert_eq!(repr(5, "none"), "");
    }
}
