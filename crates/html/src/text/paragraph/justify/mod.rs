//! Выключка при отрисовке (paint_justified): слова, полосы, видимые прогоны.

mod bands;
mod paint_justified;
mod pieces;

use gpui::Pixels;

/// Слово строки и сколько пробелов стоит перед ним от начала строки.
pub(crate) struct Word {
    pub(crate) range: std::ops::Range<usize>,
    pub(crate) spaces_before: usize,
}

/// Точка возможного разрыва.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stop {
    pub(crate) at: usize,
    pub(crate) mandatory: bool,
}

/// Какой край отрезка спрашивают: у конца строки индекс сразу за переводом
/// строки принадлежит прошлому куску, у начала — новому.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Edge {
    Start,
    End,
}

/// Кусок текста между обязательными разрывами вместе со своим набором.
#[derive(Clone)]
pub(crate) struct Seg {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) layout: std::sync::Arc<gpui::LineLayout>,
    /// Сдвиг начала куска внутри своей строки. Нужен табуляции: она рвёт
    /// набор на куски, и каждый следующий начинается со своей позиции табуляции.
    pub(crate) offset: Pixels,
}

/// Длина куска без хвостовых пробелов — они висят за краем строки.
/// Сколько байт схлопываемых пробелов в НАЧАЛЕ строки: по CSS они удаляются
/// вместе с переносом, иначе следующая строка начинается с отступа в пробел.
pub(super) fn skip_leading(chunk: &str) -> usize {
    chunk.len() - chunk.trim_start_matches([' ', '\t']).len()
}

/// Разделитель, который висит за краем строки. Неразрывный пробел сюда НЕ
/// входит: он держит слова вместе и место занимает всегда. Идеографический
/// U+3000 и прочие Zs-разделители ВИСЯТ (`trailing-ideographic-space-002`,
/// `trailing-other-space-separators-001..004`) — сужение набора до
/// 0x20/09/0A теряло 9 зелёных пар, а целевые break-spaces тесты не чинило.
pub(super) fn hangs(ch: char) -> bool {
    matches!(
        ch as u32,
        0x20 | 0x09 | 0x0A | 0x1680 | 0x2000..=0x200A | 0x202F | 0x205F | 0x3000
    )
}

/// Разделитель слов по css-text-3 §8.2: к нему прибавляется `word-spacing`,
/// и по нему же выключка раздаёт остаток строки.
///
/// Обычным пробелом набор не исчерпывается: пока неразрывный в него не
/// входил, `word-spacing` на строке из `&nbsp;` не действовал вовсе
/// (`word-spacing-001`).
/// CJK ideographs, kana and CJK symbols: `text-justify: auto` expands
/// around them (Blink `Character::IsCJKIdeographOrSymbol`).
pub(super) fn justify_ideograph(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x2FDF | 0x3001..=0x303F | 0x3040..=0x30FF | 0x31C0..=0x31FF
        | 0x3200..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0x20000..=0x3FFFF)
}

/// Cursive (joining) scripts: no inter-character expansion inside them
/// (css-text-3 §7.3 `inter-character`, §7.3.1 cursive scripts).
pub(super) fn cursive_script(c: char) -> bool {
    matches!(c as u32,
        0x0600..=0x08FF | 0x1800..=0x18AF
        | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF | 0x10D00..=0x10D3F)
}

pub(super) fn word_separator(ch: char) -> bool {
    // Идеографический пробел U+3000 — ФИКСИРОВАННОЙ ширины и разделителем
    // слов НЕ считается (css-text-3 §word-separator; word-spacing-
    // characters-001): `word-spacing` его не трогает.
    matches!(
        ch as u32,
        0x20 | 0xA0 | 0x1361 | 0x10100 | 0x10101 | 0x1039F | 0x1091F
    )
}

/// Знак управления нулевой ширины: сам не висит, но обрезка хвоста смотрит
/// сквозь него — иначе пробел перед ним перестаёт висеть.
pub(super) fn zero_width(ch: char) -> bool {
    matches!(ch as u32, 0x200B | 0x2060 | 0xFEFF | 0x202A..=0x202E | 0x2066..=0x2069)
}

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО: обрезать ПОДЛОЖКУ прогонов на висящем хвосте
/// строки (css-text §8.1: разделители у конца строки в строку не входят,
/// значит и фон строчной коробки за ними тянуться не должен). Набор при этом
/// рисовался целиком, менялись только `background_color` хвостовых прогонов.
/// Срез css-text + white-space + linebox (1730 пар, 1582 зелёных):
/// * хвост по всему `hangs` — 1573 (−9): три целевых
///   `trailing-other-space-separators-001/003/004` 0.54 → 0.00, но вся семья
///   `trailing-ideographic-space-002/017..025` уходит в «красное видно»:
///   идеографический пробел U+3000 фиксированной ширины и НЕ висит;
/// * без U+3000 — 1580 (−2): целевые три возвращаются в красное (их хвост
///   КОНЧАЕТСЯ на U+3000, обрезка об него спотыкается), ломаются
///   `line-break-anywhere-and-white-space-006/007` (`pre-wrap`) и
///   `word-spacing-characters-002`;
/// * без U+3000 и с пропуском строк, где пробелы СОХРАНЯЮТСЯ (`pre`,
///   `pre-wrap`), — 1582, ровно baseline: +`line-break-anywhere-and-white-
///   space-004`, −`word-spacing-characters-002`.
///
/// То есть висение U+3000 требуют одни пары и запрещают другие: развилка не
/// в знаке, а в том, чем кончается строка. Возвращать вместе с настоящим
/// правилом Phase II (обрезка хвоста в САМОМ разборе строки, а не в подложке).
pub(super) fn trim_hanging(chunk: &str) -> usize {
    // Идеографический пробел тоже не тянет за собой перенос: место он
    // занимает и рисуется, но строку из-за него не рвут — иначе он один
    // уезжал бы на следующую строку.
    //
    // Соединитель слов (U+FEFF) из обрезки ИСКЛЮЧЁН: им помечены распорки
    // строчных коробок, и в них лежит поле — обрезав хвостовую, коробка
    // теряла своё правое поле целиком (`word-space-transform-010`).
    chunk
        .trim_end_matches(|c| c != '\u{feff}' && (hangs(c) || zero_width(c)))
        .len()
}
