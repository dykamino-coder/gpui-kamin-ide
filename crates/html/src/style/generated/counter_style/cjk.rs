//! Позиционные CJK-стили (css-counter-styles-3 §7.1.4): таблица стилей и запись числа.

use super::*;

/// Позиционный («длинный») восточноазиатский стиль (css-counter-styles-3
/// §6.4): десять цифр, маркеры разрядов десятков/сотен/тысяч и строка знака
/// минуса. Групповые маркеры 万/億/兆 сюда не входят: обязательный диапазон
/// стиля — −9999..9999, а всё сверх него уходит в резерв (см. шапку файла).
pub(super) struct Cjk {
    pub(super) digits: [char; 10],
    /// Маркеры десятков, сотен и тысяч.
    pub(super) markers: [char; 3],
    pub(super) negative: &'static str,
    /// Единица перед маркером разряда не пишется — японский и корейский
    /// неформальные (`100` — `百`, `1000` — `千`).
    pub(super) drop_one: bool,
    /// Только китайские неформальные: у 10..19 пропадает цифра десятков, а
    /// маркер остаётся (`11` — `十一`), но `100` — всё равно `一百`.
    pub(super) drop_teen: bool,
    /// Китайские пишут внутренний ноль знаком нуля и схлопывают подряд
    /// идущие (`1001` — `一千零一`); японские и корейские просто выбрасывают
    /// (`101` — `百一`).
    pub(super) inner_zero: bool,
    /// Вне диапазона японские и китайские падают на `cjk-decimal` (эталоны
    /// `css3-counter-styles-044/049/073/078/083/088`: `一〇〇〇〇`),
    /// корейские — на десятичный (`-054/-059/-064`: `10000, `).
    pub(super) fallback_cjk: bool,
}

/// Девять позиционных стилей §6.4. Цифры и маркеры сверены со сводной
/// таблицей спеки (0, 1, 2, 3, 10, 11, 99, 100, 101, 6001) и с эталонами
/// `css3-counter-styles-042…089`.
pub(super) const CJK_STYLES: &[(&str, Cjk)] = &[
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
pub(super) fn cjk_positional(n: usize, t: &Cjk) -> String {
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
pub(super) fn cjk_repr(value: i32, t: &Cjk) -> String {
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
