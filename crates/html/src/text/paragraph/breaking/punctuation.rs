//! Punctuation for breaking; split out to keep the owning module within 250 lines.

/// Открывающий знак — скобка или кавычка.
pub(crate) fn is_opening(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        OpenPunctuation | Quotation
    )
}

/// Закрывающий знак — скобка или кавычка.
pub(crate) fn is_closing(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        ClosePunctuation | CloseParenthesis | Quotation
    )
}

/// Точка или запятая — то, что свисает по `force-end`/`allow-end`.
pub(crate) fn is_stop(ch: char) -> bool {
    matches!(
        ch,
        '.' | ','
            | '\u{060C}'
            | '\u{06D4}'
            | '、'
            | '。'
            | '，'
            | '．'
            | '\u{FE50}'
            | '\u{FE51}'
            | '\u{FE52}'
            | '\u{FF61}'
            | '\u{FF64}'
    )
}

/// Буквенная единица письма: между такими знаками `word-break: keep-all`
/// запрещает разрыв. Знаки препинания сюда не входят — после них рвать можно.
///
/// Пробел единицей письма не является НИКАКОЙ, даже идеографический: по
/// классу переноса он иероглиф (ID), и запрет заодно снимал перенос по нему
/// (`word-space-transform-013`: коробка шла одной строкой за край).
pub(super) fn letter_unit(ch: char) -> bool {
    if ch.is_whitespace() {
        return false;
    }
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        Alphabetic
            | Numeric
            | Ambiguous
            | Ideographic
            | ConditionalJapaneseStarter
            | HebrewLetter
            | ComplexContext
            | CombiningMark
            | HangulLvSyllable
            | HangulLvtSyllable
            | HangulLJamo
            | HangulVJamo
            | HangulTJamo
    )
}

/// Знак, перед которым рвать нельзя: закрывающая скобка, знак препинания,
/// разделитель разрядов, неразрывный пробел (классы UAX-14).
pub(super) fn no_break_before(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        ClosePunctuation
            | CloseParenthesis
            | Exclamation
            | InfixSeparator
            | NonStarter
            | Symbol
            | NonBreakingGlue
            | WordJoiner
            | ZeroWidthJoiner
    )
}

/// Знак, после которого рвать нельзя: открывающая скобка, склейка, знак
/// перед числом (`$`, `\`, `£`).
///
/// Соединитель нулевой ширины держит составные знаки вместе — на нём собраны
/// целые эмодзи (человек + компьютер = «программист»). Разрыв по нему
/// рассыпал бы один знак на составные части.
pub(super) fn no_break_after(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        OpenPunctuation | NonBreakingGlue | WordJoiner | ZeroWidthJoiner | Prefix
    )
}

/// Центрированная пунктуация (css-text-3 §5.2, `loose` в ja/zh): U+30FB,
/// U+FF1A, U+FF1B, U+FF65, U+203C, U+2047-2049, U+FF01, U+FF1F.
pub(super) fn centered_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '\u{30FB}' | '\u{FF1A}' | '\u{FF1B}' | '\u{FF65}' | '\u{203C}' | '\u{2047}'
            ..='\u{2049}' | '\u{FF01}' | '\u{FF1F}'
    )
}

/// Постфиксы класса PO с восточноазиатской шириной A/F/W (§5.2, `loose` в
/// ja/zh): °, ‰, ℃, ％ и полноширинные знаки процента/цента.
pub(super) fn wide_postfix(ch: char) -> bool {
    matches!(
        ch,
        '\u{00B0}'
            | '\u{2030}'
            | '\u{2031}'
            | '\u{2103}'
            | '\u{2109}'
            | '\u{FF05}'
            | '\u{FFE0}'
            | '\u{2032}'
            | '\u{2033}'
    )
}

/// Префиксы класса PR с шириной A/F/W (§5.2, `loose` в ja/zh): €, №, ￥, ￡,
/// ＄, ₩, §, ¶.
pub(super) fn wide_prefix(ch: char) -> bool {
    matches!(
        ch,
        '\u{20AC}'
            | '\u{2116}'
            | '\u{FFE5}'
            | '\u{FFE1}'
            | '\u{FF04}'
            | '\u{FFE6}'
            | '\u{00A7}'
            | '\u{00B6}'
            | '\u{20A9}'
    )
}

/// Класс CJ по UAX-14: малая кана, знак долготы, их полуширинные формы.
/// `unicode_linebreak` разрешает CJ как NS (строгий вариант) — перед ними
/// разрыва нет; `line-break: normal`/`loose` его возвращают (css-text-3 §5.2:
/// «breaks before Japanese small kana or the Katakana-Hiragana prolonged
/// sound mark, i.e. characters from the Unicode line breaking class CJ»).
pub(super) fn conditional_japanese_starter(ch: char) -> bool {
    matches!(
        ch,
        '\u{3041}' | '\u{3043}' | '\u{3045}' | '\u{3047}' | '\u{3049}' | '\u{3063}'
            | '\u{3083}' | '\u{3085}' | '\u{3087}' | '\u{308E}' | '\u{3095}' | '\u{3096}'
            | '\u{30A1}' | '\u{30A3}' | '\u{30A5}' | '\u{30A7}' | '\u{30A9}' | '\u{30C3}'
            | '\u{30E3}' | '\u{30E5}' | '\u{30E7}' | '\u{30EE}' | '\u{30F5}' | '\u{30F6}'
            | '\u{30FC}'
            | '\u{31F0}'..='\u{31FF}'
            | '\u{FF67}'..='\u{FF70}'
    )
}

/// Знаки повтора (css-text-3 §5.2, только `loose`): U+3005, U+303B, U+309D,
/// U+309E, U+30FD, U+30FE.
pub(super) fn iteration_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{3005}' | '\u{303B}' | '\u{309D}' | '\u{309E}' | '\u{30FD}' | '\u{30FE}'
    )
}
