//! Числовые и алфавитные системы встроенных стилей счётчика: roman, alphabetic, numeric, аддитивная запись, наборы цифр и азбук.

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
pub(super) fn alphabetic(mut n: usize, base: u8) -> String {
    let mut out = vec![];
    while n > 0 {
        let rem = (n - 1) % 26;
        out.push((base + rem as u8) as char);
        n = (n - 1) / 26;
    }
    out.iter().rev().collect()
}

/// Греческая строчная: α, β, γ… (без ς — css-counter-styles §lower-greek).
pub(super) fn lower_greek(n: usize) -> String {
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
pub(super) fn numeric(value: i32, digits: &[char]) -> String {
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
pub(super) fn alphabetic_of(mut n: usize, letters: &[char]) -> String {
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
pub(super) const NUMERIC_ZERO: &[(&str, char)] = &[
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
pub(super) const CJK_DIGITS: [char; 10] =
    ['〇', '一', '二', '三', '四', '五', '六', '七', '八', '九'];

/// Кана и циклы (css-counter-styles-3 §6.2, §6.3). Порядок сверен с тестами
/// `css3-counter-styles-030/033/036/039/201/204`.
pub(super) const HIRAGANA: &str = "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわゐゑをん";

pub(super) const HIRAGANA_IROHA: &str = "いろはにほへとちりぬるをわかよたれそつねならむうゐのおくやまけふこえてあさきゆめみしゑひもせす";

pub(super) const KATAKANA: &str = "アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヰヱヲン";

pub(super) const KATAKANA_IROHA: &str = "イロハニホヘトチリヌルヲワカヨタレソツネナラムウヰノオクヤマケフコエテアサキユメミシヱヒモセス";

pub(super) const EARTHLY_BRANCH: &str = "子丑寅卯辰巳午未申酉戌亥";

pub(super) const HEAVENLY_STEM: &str = "甲乙丙丁戊己庚辛壬癸";

/// Аддитивная система (css-counter-styles-3 §additive): значение
/// набирается из наибольших подходящих знаков подряд.
pub(super) fn additive(mut n: usize, table: &[(usize, char)]) -> String {
    let mut out = String::new();
    for (weight, sign) in table {
        while n >= *weight {
            out.push(*sign);
            n -= weight;
        }
    }
    out
}

/// То же, но знак веса набран из НЕСКОЛЬКИХ букв: у иврита тысяча пишется
/// буквой с герешем (`1000` — `א׳`), а 15 и 16 — особыми парами.
pub(super) fn additive_str(mut n: usize, table: &[(usize, &str)]) -> String {
    let mut out = String::new();
    for (weight, sign) in table {
        while n >= *weight {
            out.push_str(sign);
            n -= weight;
        }
    }
    out
}
