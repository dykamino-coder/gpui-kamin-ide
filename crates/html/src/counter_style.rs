//! ★ ЗАМЕР 05.09 (+0/−3) БЫЛ ЛОЖНЫМ: эталоны семьи `css3-counter-styles-*`
//! рисовались ПУСТЫМИ (не-`li` дети `<ol>` выбрасывались в `render.rs::list`),
//! и таблицы знаков было не с чем сравнивать. После починки эталона (06.09)
//! те же таблицы дали +48/−11 на срезе 501 пары — см. `scout-counterstyles-
//! 2026-09.md`. Все 11 потерь были позиционными CJK; их закрывает
//! `cjk_positional` ниже (`scout-counterstyles-2026-09b.md`).
//!
//! ★ ДИАПАЗОНЫ ВЗЯТЫ ОБЯЗАТЕЛЬНЫЕ, А НЕ РАСШИРЕННЫЕ. Восточноазиатские —
//! −9999..9999 (css-counter-styles-3 §6.4 «Limited-range Implementation
//! (required)»), иврит — 1..10999 (§hebrew). Необязательное расширение
//! (§extended-range-optional до 10^16, иврит до 999999, как в Blink)
//! ПРОВАЛИВАЕТ эталоны `css3-counter-styles-044/049/054/059/064/073/078/
//! 083/088/016a`, которые ждут именно резерва; расширенные эталоны лежат
//! рядом в НЕиспользуемых базой `*-alt-ref.html`. Второе препятствие:
//! значение счётчика в крейте — `i32` (`counters.rs`), а расширенные тесты
//! набирают 10^12..10^16.
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

/// То же, но знак веса набран из НЕСКОЛЬКИХ букв: у иврита тысяча пишется
/// буквой с герешем (`1000` — `א׳`), а 15 и 16 — особыми парами.
fn additive_str(mut n: usize, table: &[(usize, &str)]) -> String {
    let mut out = String::new();
    for (weight, sign) in table {
        while n >= *weight {
            out.push_str(sign);
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

/// Иврит, 1..10999 (css-counter-styles-3 §hebrew). 15 и 16 записаны
/// отдельными парами `טו`/`טז`: обычное сложение дало бы сочетание, слишком
/// похожее на тетраграмматон. Тысячи — буква с герешем `׳` U+05F3
/// (`1000` — `א׳`, `9999` — `ט׳תתקצט`).
const HEBREW: &[(usize, &str)] = &[
    (10000, "י׳"),
    (9000, "ט׳"),
    (8000, "ח׳"),
    (7000, "ז׳"),
    (6000, "ו׳"),
    (5000, "ה׳"),
    (4000, "ד׳"),
    (3000, "ג׳"),
    (2000, "ב׳"),
    (1000, "א׳"),
    (400, "ת"),
    (300, "ש"),
    (200, "ר"),
    (100, "ק"),
    (90, "צ"),
    (80, "פ"),
    (70, "ע"),
    (60, "ס"),
    (50, "נ"),
    (40, "מ"),
    (30, "ל"),
    (20, "כ"),
    (19, "יט"),
    (18, "יח"),
    (17, "יז"),
    (16, "טז"),
    (15, "טו"),
    (10, "י"),
    (9, "ט"),
    (8, "ח"),
    (7, "ז"),
    (6, "ו"),
    (5, "ה"),
    (4, "ד"),
    (3, "ג"),
    (2, "ב"),
    (1, "א"),
];

/// Эфиопские цифры единиц (1..9) и десятков (10..90).
const ETHIOPIC_UNITS: [char; 9] = ['፩', '፪', '፫', '፬', '፭', '፮', '፯', '፰', '፱'];
const ETHIOPIC_TENS: [char; 9] = ['፲', '፳', '፴', '፵', '፶', '፷', '፸', '፹', '፺'];

/// `ethiopic-numeric` (css-counter-styles-3 §ethiopic-numeric): число делится
/// на группы по ДВЕ цифры, чётные группы отделяются `፼` U+137C, нечётные —
/// `፻` U+137B. Цифры группы не пишутся, если группа нулевая, если она старшая
/// и равна единице или если она нечётная и равна единице (`100` — `፻`, а не
/// `፩፻`). Разделитель нулевой группы приписывается безусловно и снимается в
/// конце.
fn ethiopic(mut n: usize) -> String {
    if n < 10 {
        return ETHIOPIC_UNITS[n - 1].to_string();
    }
    let mut rev = Vec::new();
    let mut odd = false;
    while n > 0 {
        let group = n % 100;
        n /= 100;
        if odd {
            if group != 0 {
                rev.push('፻');
            }
        } else {
            rev.push('፼');
        }
        let bare = group == 0 || (group == 1 && (n == 0 || odd));
        if !bare {
            if group % 10 != 0 {
                rev.push(ETHIOPIC_UNITS[group % 10 - 1]);
            }
            if group / 10 != 0 {
                rev.push(ETHIOPIC_TENS[group / 10 - 1]);
            }
        }
        odd = !odd;
    }
    rev.reverse();
    rev.pop();
    rev.into_iter().collect()
}

/// Позиционный («длинный») восточноазиатский стиль (css-counter-styles-3
/// §6.4): десять цифр, маркеры разрядов десятков/сотен/тысяч и строка знака
/// минуса. Групповые маркеры 万/億/兆 сюда не входят: обязательный диапазон
/// стиля — −9999..9999, а всё сверх него уходит в резерв (см. шапку файла).
struct Cjk {
    digits: [char; 10],
    /// Маркеры десятков, сотен и тысяч.
    markers: [char; 3],
    negative: &'static str,
    /// Единица перед маркером разряда не пишется — японский и корейский
    /// неформальные (`100` — `百`, `1000` — `千`).
    drop_one: bool,
    /// Только китайские неформальные: у 10..19 пропадает цифра десятков, а
    /// маркер остаётся (`11` — `十一`), но `100` — всё равно `一百`.
    drop_teen: bool,
    /// Китайские пишут внутренний ноль знаком нуля и схлопывают подряд
    /// идущие (`1001` — `一千零一`); японские и корейские просто выбрасывают
    /// (`101` — `百一`).
    inner_zero: bool,
    /// Вне диапазона японские и китайские падают на `cjk-decimal` (эталоны
    /// `css3-counter-styles-044/049/073/078/083/088`: `一〇〇〇〇`),
    /// корейские — на десятичный (`-054/-059/-064`: `10000, `).
    fallback_cjk: bool,
}

/// Девять позиционных стилей §6.4. Цифры и маркеры сверены со сводной
/// таблицей спеки (0, 1, 2, 3, 10, 11, 99, 100, 101, 6001) и с эталонами
/// `css3-counter-styles-042…089`.
const CJK_STYLES: &[(&str, Cjk)] = &[
    (
        "japanese-informal",
        Cjk {
            digits: ['〇', '一', '二', '三', '四', '五', '六', '七', '八', '九'],
            markers: ['十', '百', '千'],
            negative: "マイナス",
            drop_one: true,
            drop_teen: false,
            inner_zero: false,
            fallback_cjk: true,
        },
    ),
    (
        "japanese-formal",
        Cjk {
            digits: ['零', '壱', '弐', '参', '四', '伍', '六', '七', '八', '九'],
            markers: ['拾', '百', '阡'],
            negative: "マイナス",
            drop_one: false,
            drop_teen: false,
            inner_zero: false,
            fallback_cjk: true,
        },
    ),
    (
        "korean-hangul-formal",
        Cjk {
            digits: ['영', '일', '이', '삼', '사', '오', '육', '칠', '팔', '구'],
            markers: ['십', '백', '천'],
            negative: "마이너스 ",
            drop_one: false,
            drop_teen: false,
            inner_zero: false,
            fallback_cjk: false,
        },
    ),
    (
        "korean-hanja-informal",
        Cjk {
            digits: ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'],
            markers: ['十', '百', '千'],
            negative: "마이너스 ",
            drop_one: true,
            drop_teen: false,
            inner_zero: false,
            fallback_cjk: false,
        },
    ),
    (
        "korean-hanja-formal",
        Cjk {
            digits: ['零', '壹', '貳', '參', '四', '五', '六', '七', '八', '九'],
            markers: ['拾', '百', '仟'],
            negative: "마이너스 ",
            drop_one: false,
            drop_teen: false,
            inner_zero: false,
            fallback_cjk: false,
        },
    ),
    (
        "simp-chinese-informal",
        Cjk {
            digits: ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'],
            markers: ['十', '百', '千'],
            negative: "负",
            drop_one: false,
            drop_teen: true,
            inner_zero: true,
            fallback_cjk: true,
        },
    ),
    (
        "simp-chinese-formal",
        Cjk {
            digits: ['零', '壹', '贰', '叁', '肆', '伍', '陆', '柒', '捌', '玖'],
            markers: ['拾', '佰', '仟'],
            negative: "负",
            drop_one: false,
            drop_teen: false,
            inner_zero: true,
            fallback_cjk: true,
        },
    ),
    (
        "trad-chinese-informal",
        Cjk {
            digits: ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'],
            markers: ['十', '百', '千'],
            negative: "負",
            drop_one: false,
            drop_teen: true,
            inner_zero: true,
            fallback_cjk: true,
        },
    ),
    (
        "trad-chinese-formal",
        Cjk {
            digits: ['零', '壹', '貳', '參', '肆', '伍', '陸', '柒', '捌', '玖'],
            markers: ['拾', '佰', '仟'],
            negative: "負",
            drop_one: false,
            drop_teen: false,
            inner_zero: true,
            fallback_cjk: true,
        },
    ),
];

/// Позиционная запись 0..9999 знаками стиля (css-counter-styles-3
/// §limited-chinese; аддитивные таблицы японских и корейских стилей §6.4 в
/// этом диапазоне дают ровно то же самое, поэтому алгоритм один на всех).
///
/// Слоты записи фиксированы: цифра тысяч, маркер тысяч, цифра сотен, маркер
/// сотен, цифра десятков, маркер десятков, цифра единиц. Разряд заполняется,
/// только если число до него дотянулось (шаг 3 алгоритма), после чего
/// действуют «сброс единицы» и «сброс нулей» своего стиля (шаги 4-5).
fn cjk_positional(n: usize, t: &Cjk) -> String {
    if n == 0 {
        return t.digits[0].to_string();
    }
    let mut slot: [Option<char>; 7] = [None; 7];
    let ones = n % 10;
    if ones != 0 {
        slot[6] = Some(t.digits[ones]);
    }
    // Ноль пишется знаком нуля только ВНУТРИ числа: пока справа одни нули,
    // писать нечего («drop any trailing zeros»).
    let mut trailing_zero = ones == 0;
    let mut div = 10;
    for (step, threshold) in [9usize, 99, 999].into_iter().enumerate() {
        if n <= threshold {
            break;
        }
        let digit = n / div % 10;
        div *= 10;
        let at = 4 - step * 2;
        if digit == 0 {
            if t.inner_zero && !trailing_zero {
                slot[at] = Some(t.digits[0]);
            }
        } else {
            if !(t.drop_one && digit == 1) {
                slot[at] = Some(t.digits[digit]);
            }
            slot[at + 1] = Some(t.markers[step]);
        }
        trailing_zero &= digit == 0;
    }
    if t.drop_teen && n < 20 {
        slot[4] = None;
    }
    // Подряд идущие нули схлопываются в один. Хвостового нуля тут быть не
    // может: знак нуля ставится, лишь когда правее уже есть ненулевая цифра.
    let mut out = String::new();
    let mut was_zero = false;
    for sign in slot.into_iter().flatten() {
        let zero = sign == t.digits[0];
        if !(zero && was_zero) {
            out.push(sign);
        }
        was_zero = zero;
    }
    out
}

/// Позиционный стиль целиком: знак минуса своей строкой (`negative` §6.4) и
/// обязательный диапазон −9999..9999, вне которого берётся резерв стиля.
fn cjk_repr(value: i32, t: &Cjk) -> String {
    match value {
        0..=9999 => cjk_positional(value.unsigned_abs() as usize, t),
        -9999..=-1 => format!(
            "{}{}",
            t.negative,
            cjk_positional(value.unsigned_abs() as usize, t)
        ),
        _ if t.fallback_cjk => numeric(value, &CJK_DIGITS),
        _ => value.to_string(),
    }
}

/// Описание предопределённого стиля как набора дескрипторов `@counter-style`
/// (css-counter-styles-3 §6–§7): его наследует `system: extends <имя>`.
pub(crate) struct Builtin {
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
pub(crate) fn is_predefined(name: &str) -> bool {
    matches!(
        name,
        "decimal" | "decimal-leading-zero" | "disc" | "circle" | "square"
            | "disclosure-open" | "disclosure-closed" | "lower-roman" | "upper-roman"
            | "lower-alpha" | "lower-latin" | "upper-alpha" | "upper-latin" | "lower-greek"
            | "cjk-decimal" | "hiragana" | "hiragana-iroha" | "katakana" | "katakana-iroha"
            | "cjk-earthly-branch" | "cjk-heavenly-stem" | "lower-armenian" | "armenian"
            | "upper-armenian" | "georgian" | "hebrew" | "ethiopic-numeric" | "cjk-ideographic"
    ) || NUMERIC_ZERO.iter().any(|(k, _)| *k == name)
        || CJK_STYLES.iter().any(|(k, _)| *k == name)
}

/// Имя стиля, как его видит каскад: предопределённое — строчными.
pub(crate) fn normalize_name(name: &str) -> std::borrow::Cow<'_, str> {
    if name.bytes().any(|b| b.is_ascii_uppercase()) {
        let low = name.to_ascii_lowercase();
        if is_predefined(&low) {
            return std::borrow::Cow::Owned(low);
        }
    }
    std::borrow::Cow::Borrowed(name)
}

pub(crate) fn builtin(name: &str) -> Option<Builtin> {
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
        "lower-alpha" | "lower-latin" | "upper-alpha" | "upper-latin" | "lower-greek"
        | "hiragana" | "hiragana-iroha" | "katakana" | "katakana-iroha" | "ethiopic-numeric" => {
            Some((1, i64::MAX))
        }
        _ if cjk.is_some() => Some((-9999, 9999)),
        _ => None,
    };
    let fallback = match name {
        "cjk-earthly-branch" | "cjk-heavenly-stem" => "cjk-decimal",
        _ if cjk.is_some_and(|t| t.fallback_cjk) => "cjk-decimal",
        _ => "decimal",
    };
    let uses_negative = !matches!(
        name,
        "disc" | "circle" | "square" | "disclosure-open" | "disclosure-closed"
            | "cjk-earthly-branch" | "cjk-heavenly-stem"
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
pub(crate) fn builtin_initial(name: &str, n: u64) -> Option<String> {
    let v = i32::try_from(n).ok()?;
    let positive = usize::try_from(n).ok().filter(|n| *n > 0);
    Some(match name {
        "decimal" | "decimal-leading-zero" => n.to_string(),
        "disc" => "•".to_string(),
        "circle" => "◦".to_string(),
        "square" => "▪".to_string(),
        "lower-roman" => roman(positive?),
        "upper-roman" => roman(positive?).to_uppercase(),
        "lower-alpha" | "lower-latin" => alphabetic(positive?, b'a'),
        "upper-alpha" | "upper-latin" => alphabetic(positive?, b'A'),
        "lower-greek" => lower_greek(positive?),
        "cjk-decimal" => numeric(v, &CJK_DIGITS),
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
                let digits: Vec<char> =
                    (0..10).filter_map(|d| char::from_u32(*zero as u32 + d)).collect();
                numeric(v, &digits)
            } else {
                let t = if name == "cjk-ideographic" {
                    &CJK_STYLES[7].1
                } else {
                    &CJK_STYLES.iter().find(|(k, _)| *k == name)?.1
                };
                if n > 9999 {
                    return None;
                }
                cjk_positional(n as usize, t)
            }
        }
    })
}

/// Значение счётчика знаками названного стиля.
///
/// Незнакомый стиль — десятичный (css-counter-styles §counter-style-name:
/// неизвестное имя ведёт себя как `decimal`). Стили с конечным диапазоном
/// (римский, алфавитные) вне его тоже падают на десятичный.
pub fn repr(value: i32, style: &str) -> String {
    let style = &*normalize_name(style);
    if let Some(s) = crate::counter_style_rules::custom_repr(value as i64, style) {
        return s;
    }
    builtin_repr(value, style)
}

/// Предопределённый стиль (или десятичный для незнакомого имени) без учёта
/// правил `@counter-style` документа.
pub(crate) fn builtin_repr(value: i32, style: &str) -> String {
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
        // Иврит — аддитив с диапазоном 1..10999 (§hebrew). Расширение до
        // 999999, как в Blink, ПРОВАЛИВАЕТ `css3-counter-styles-016a`: её
        // эталон ждёт на 11000 десятичный резерв (расширенный вариант лежит
        // в неиспользуемом базой `-016a-alt-ref.html`).
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
        "cjk-decimal" | "hiragana" | "hiragana-iroha" | "katakana" | "katakana-iroha"
        | "cjk-earthly-branch" | "cjk-heavenly-stem" | "japanese-formal"
        | "japanese-informal" | "simp-chinese-formal" | "simp-chinese-informal"
        | "trad-chinese-formal" | "trad-chinese-informal" => "、",
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
pub fn marker_repr(value: i32, style: &str) -> String {
    if style == "none" {
        return String::new();
    }
    let style = &*normalize_name(style);
    if let Some(s) = crate::counter_style_rules::custom_marker(value as i64, style) {
        return s;
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
        assert_eq!(repr(5, "no-such-style"), "5");
        assert_eq!(repr(5, "none"), "");
    }

    #[test]
    fn hebrew_and_ethiopic_follow_spec() {
        // 15 и 16 — особые пары, тысячи — с герешем
        // (`css3-counter-styles-016`).
        assert_eq!(repr(15, "hebrew"), "טו");
        assert_eq!(repr(16, "hebrew"), "טז");
        assert_eq!(repr(11, "hebrew"), "יא");
        assert_eq!(repr(997, "hebrew"), "תתקצז");
        assert_eq!(repr(1000, "hebrew"), "א׳");
        assert_eq!(repr(3256, "hebrew"), "ג׳רנו");
        assert_eq!(repr(9999, "hebrew"), "ט׳תתקצט");
        assert_eq!(repr(10997, "hebrew"), "י׳תתקצז");
        assert_eq!(repr(11000, "hebrew"), "11000", "range: 1 10999");
        // `counter-ethiopic-numeric` (все значения его эталона).
        assert_eq!(repr(1, "ethiopic-numeric"), "፩");
        assert_eq!(repr(10, "ethiopic-numeric"), "፲");
        assert_eq!(repr(11, "ethiopic-numeric"), "፲፩");
        assert_eq!(repr(100, "ethiopic-numeric"), "፻");
        assert_eq!(repr(1005, "ethiopic-numeric"), "፲፻፭");
        assert_eq!(repr(1800, "ethiopic-numeric"), "፲፰፻");
        assert_eq!(repr(9999, "ethiopic-numeric"), "፺፱፻፺፱");
        assert_eq!(repr(10000, "ethiopic-numeric"), "፼");
        assert_eq!(repr(1000001, "ethiopic-numeric"), "፻፼፩");
        assert_eq!(repr(78010092, "ethiopic-numeric"), "፸፰፻፩፼፺፪");
        assert_eq!(repr(0, "ethiopic-numeric"), "0");
    }

    #[test]
    fn cjk_positional_follows_spec() {
        // Сводная таблица §6.4 (0, 1, 10, 11, 99, 100, 101, 6001).
        assert_eq!(repr(0, "japanese-informal"), "〇");
        assert_eq!(repr(10, "japanese-informal"), "十");
        assert_eq!(repr(100, "japanese-informal"), "百");
        assert_eq!(repr(101, "japanese-informal"), "百一");
        assert_eq!(repr(6001, "japanese-informal"), "六千一");
        assert_eq!(repr(0, "japanese-formal"), "零");
        assert_eq!(repr(10, "japanese-formal"), "壱拾");
        assert_eq!(repr(101, "japanese-formal"), "壱百壱");
        assert_eq!(repr(6001, "japanese-formal"), "六阡壱");
        assert_eq!(repr(10, "korean-hangul-formal"), "일십");
        assert_eq!(repr(101, "korean-hangul-formal"), "일백일");
        assert_eq!(repr(6001, "korean-hangul-formal"), "육천일");
        assert_eq!(repr(101, "korean-hanja-informal"), "百一");
        assert_eq!(repr(6001, "korean-hanja-formal"), "六仟壹");
        // Китайские: единица перед маркером остаётся, зато 10..19 без неё, а
        // внутренние нули пишутся знаком нуля и схлопываются.
        assert_eq!(repr(10, "simp-chinese-informal"), "十");
        assert_eq!(repr(11, "simp-chinese-informal"), "十一");
        assert_eq!(repr(20, "simp-chinese-informal"), "二十");
        assert_eq!(repr(100, "simp-chinese-informal"), "一百");
        assert_eq!(repr(101, "simp-chinese-informal"), "一百零一");
        assert_eq!(repr(6001, "simp-chinese-informal"), "六千零一");
        assert_eq!(repr(1001, "trad-chinese-informal"), "一千零一");
        assert_eq!(repr(11, "simp-chinese-formal"), "壹拾壹");
        assert_eq!(repr(99, "trad-chinese-formal"), "玖拾玖");
        assert_eq!(repr(6001, "trad-chinese-formal"), "陸仟零壹");
        // Знак минуса — своей строкой (`css3-counter-styles-045/055/084`).
        assert_eq!(repr(-11, "japanese-informal"), "マイナス十一");
        assert_eq!(repr(-11, "korean-hangul-formal"), "마이너스 일십일");
        assert_eq!(repr(-11, "trad-chinese-informal"), "負十一");
        // Вне −9999..9999: японские и китайские — `cjk-decimal`, корейские —
        // десятичный (`-049/-073` против `-054/-059/-064`).
        assert_eq!(repr(10000, "japanese-formal"), "一〇〇〇〇");
        assert_eq!(repr(10001, "simp-chinese-informal"), "一〇〇〇一");
        assert_eq!(repr(10000, "korean-hangul-formal"), "10000");
        // Наследный синоним.
        assert_eq!(repr(11, "cjk-ideographic"), "十一");
    }
}
