//! Стили счётчиков: таблицы знаков девяти позиционных восточноазиатских
//! стилей (CSS Counter Styles 3 §6.4, §7.1.5) — цифры, маркеры разрядов,
//! группы, знак минуса и признаки записи (`drop_one`, `drop_teen`, `inner_zero`).

pub(super) struct Cjk {
    pub(super) digits: [char; 10],
    /// Маркеры десятков, сотен и тысяч.
    pub(super) markers: [char; 3],
    pub(super) groups: [&'static str; 4],
    pub(super) korean: bool,
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
            groups: ["", "万", "億", "兆"],
            korean: false,
            negative: "マイナス",
            drop_one: true,
            drop_teen: false,
            inner_zero: false,
        },
    ),
    (
        "japanese-formal",
        Cjk {
            digits: ['零', '壱', '弐', '参', '四', '伍', '六', '七', '八', '九'],
            markers: ['拾', '百', '阡'],
            groups: ["", "萬", "億", "兆"],
            korean: false,
            negative: "マイナス",
            drop_one: false,
            drop_teen: false,
            inner_zero: false,
        },
    ),
    (
        "korean-hangul-formal",
        Cjk {
            digits: ['영', '일', '이', '삼', '사', '오', '육', '칠', '팔', '구'],
            markers: ['십', '백', '천'],
            groups: ["", "만", "억", "조"],
            korean: true,
            negative: "마이너스 ",
            drop_one: false,
            drop_teen: false,
            inner_zero: false,
        },
    ),
    (
        "korean-hanja-informal",
        Cjk {
            digits: ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'],
            markers: ['十', '百', '千'],
            groups: ["", "萬", "億", "兆"],
            korean: true,
            negative: "마이너스 ",
            drop_one: true,
            drop_teen: false,
            inner_zero: false,
        },
    ),
    (
        "korean-hanja-formal",
        Cjk {
            digits: ['零', '壹', '貳', '參', '四', '五', '六', '七', '八', '九'],
            markers: ['拾', '百', '仟'],
            groups: ["", "萬", "億", "兆"],
            korean: true,
            negative: "마이너스 ",
            drop_one: false,
            drop_teen: false,
            inner_zero: false,
        },
    ),
    (
        "simp-chinese-informal",
        Cjk {
            digits: ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'],
            markers: ['十', '百', '千'],
            groups: ["", "万", "亿", "万亿"],
            korean: false,
            negative: "负",
            drop_one: false,
            drop_teen: true,
            inner_zero: true,
        },
    ),
    (
        "simp-chinese-formal",
        Cjk {
            digits: ['零', '壹', '贰', '叁', '肆', '伍', '陆', '柒', '捌', '玖'],
            markers: ['拾', '佰', '仟'],
            groups: ["", "万", "亿", "万亿"],
            korean: false,
            negative: "负",
            drop_one: false,
            drop_teen: false,
            inner_zero: true,
        },
    ),
    (
        "trad-chinese-informal",
        Cjk {
            digits: ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'],
            markers: ['十', '百', '千'],
            groups: ["", "萬", "億", "兆"],
            korean: false,
            negative: "負",
            drop_one: false,
            drop_teen: true,
            inner_zero: true,
        },
    ),
    (
        "trad-chinese-formal",
        Cjk {
            digits: ['零', '壹', '貳', '參', '肆', '伍', '陸', '柒', '捌', '玖'],
            markers: ['拾', '佰', '仟'],
            groups: ["", "萬", "億", "兆"],
            korean: false,
            negative: "負",
            drop_one: false,
            drop_teen: false,
            inner_zero: true,
        },
    ),
];
