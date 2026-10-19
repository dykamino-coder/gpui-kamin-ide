//! Тесты стилей счётчиков (style::generated::counter_style): встроенные системы и представления.

use super::*;

#[test]
fn styles_follow_spec() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(repr(4, "lower-roman", vcx), "iv");
    assert_eq!(repr(4, "upper-roman", vcx), "IV");
    assert_eq!(repr(27, "lower-alpha", vcx), "aa");
    assert_eq!(repr(1, "upper-latin", vcx), "A");
    assert_eq!(repr(3, "lower-greek", vcx), "γ");
    assert_eq!(repr(7, "decimal-leading-zero", vcx), "07");
    assert_eq!(repr(-7, "decimal-leading-zero", vcx), "-07");
    assert_eq!(repr(12, "decimal-leading-zero", vcx), "12");
    // Вне диапазона стиля — десятичный резерв.
    assert_eq!(repr(0, "lower-roman", vcx), "0");
    assert_eq!(repr(-3, "upper-alpha", vcx), "-3");
    assert_eq!(repr(1, "armenian", vcx), "Ա");
    assert_eq!(repr(9999, "armenian", vcx), "ՔՋՂԹ");
    assert_eq!(
        repr(10000, "armenian", vcx),
        "10000",
        "вне диапазона — десятичный"
    );
    assert_eq!(repr(1, "georgian", vcx), "ა");
    assert_eq!(repr(19999, "georgian", vcx), "ჵჰშჟთ");
    assert_eq!(repr(20000, "georgian", vcx), "20000");
    // Незнакомое имя ведёт себя как decimal.
    assert_eq!(repr(5, "no-such-style", vcx), "5");
    assert_eq!(repr(5, "none", vcx), "");
}

#[test]
fn hebrew_and_ethiopic_follow_spec() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    // 15 и 16 — особые пары, тысячи — с герешем
    // (`css3-counter-styles-016`).
    assert_eq!(repr(15, "hebrew", vcx), "טו");
    assert_eq!(repr(16, "hebrew", vcx), "טז");
    assert_eq!(repr(11, "hebrew", vcx), "יא");
    assert_eq!(repr(997, "hebrew", vcx), "תתקצז");
    assert_eq!(repr(1000, "hebrew", vcx), "א׳");
    assert_eq!(repr(3256, "hebrew", vcx), "ג׳רנו");
    assert_eq!(repr(9999, "hebrew", vcx), "ט׳תתקצט");
    assert_eq!(repr(10997, "hebrew", vcx), "י׳תתקצז");
    assert_eq!(repr(11000, "hebrew", vcx), "11000", "range: 1 10999");
    // `counter-ethiopic-numeric` (все значения его эталона).
    assert_eq!(repr(1, "ethiopic-numeric", vcx), "፩");
    assert_eq!(repr(10, "ethiopic-numeric", vcx), "፲");
    assert_eq!(repr(11, "ethiopic-numeric", vcx), "፲፩");
    assert_eq!(repr(100, "ethiopic-numeric", vcx), "፻");
    assert_eq!(repr(1005, "ethiopic-numeric", vcx), "፲፻፭");
    assert_eq!(repr(1800, "ethiopic-numeric", vcx), "፲፰፻");
    assert_eq!(repr(9999, "ethiopic-numeric", vcx), "፺፱፻፺፱");
    assert_eq!(repr(10000, "ethiopic-numeric", vcx), "፼");
    assert_eq!(repr(1000001, "ethiopic-numeric", vcx), "፻፼፩");
    assert_eq!(repr(78010092, "ethiopic-numeric", vcx), "፸፰፻፩፼፺፪");
    assert_eq!(repr(0, "ethiopic-numeric", vcx), "0");
}

#[test]
fn cjk_positional_follows_spec() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    // Сводная таблица §6.4 (0, 1, 10, 11, 99, 100, 101, 6001).
    assert_eq!(repr(0, "japanese-informal", vcx), "〇");
    assert_eq!(repr(10, "japanese-informal", vcx), "十");
    assert_eq!(repr(100, "japanese-informal", vcx), "百");
    assert_eq!(repr(101, "japanese-informal", vcx), "百一");
    assert_eq!(repr(6001, "japanese-informal", vcx), "六千一");
    assert_eq!(repr(0, "japanese-formal", vcx), "零");
    assert_eq!(repr(10, "japanese-formal", vcx), "壱拾");
    assert_eq!(repr(101, "japanese-formal", vcx), "壱百壱");
    assert_eq!(repr(6001, "japanese-formal", vcx), "六阡壱");
    assert_eq!(repr(10, "korean-hangul-formal", vcx), "일십");
    assert_eq!(repr(101, "korean-hangul-formal", vcx), "일백일");
    assert_eq!(repr(6001, "korean-hangul-formal", vcx), "육천일");
    assert_eq!(repr(101, "korean-hanja-informal", vcx), "百一");
    assert_eq!(repr(6001, "korean-hanja-formal", vcx), "六仟壹");
    // Китайские: единица перед маркером остаётся, зато 10..19 без неё, а
    // внутренние нули пишутся знаком нуля и схлопываются.
    assert_eq!(repr(10, "simp-chinese-informal", vcx), "十");
    assert_eq!(repr(11, "simp-chinese-informal", vcx), "十一");
    assert_eq!(repr(20, "simp-chinese-informal", vcx), "二十");
    assert_eq!(repr(100, "simp-chinese-informal", vcx), "一百");
    assert_eq!(repr(101, "simp-chinese-informal", vcx), "一百零一");
    assert_eq!(repr(6001, "simp-chinese-informal", vcx), "六千零一");
    assert_eq!(repr(1001, "trad-chinese-informal", vcx), "一千零一");
    assert_eq!(repr(11, "simp-chinese-formal", vcx), "壹拾壹");
    assert_eq!(repr(99, "trad-chinese-formal", vcx), "玖拾玖");
    assert_eq!(repr(6001, "trad-chinese-formal", vcx), "陸仟零壹");
    // Знак минуса — своей строкой (`css3-counter-styles-045/055/084`).
    assert_eq!(repr(-11, "japanese-informal", vcx), "マイナス十一");
    assert_eq!(repr(-11, "korean-hangul-formal", vcx), "마이너스 일십일");
    assert_eq!(repr(-11, "trad-chinese-informal", vcx), "負十一");
    // Extended ranges, CSS Counter Styles 3 section 7.1.5.
    assert_eq!(repr(10000, "japanese-formal", vcx), "壱萬");
    assert_eq!(repr(10001, "simp-chinese-informal", vcx), "一万零一");
    assert_eq!(repr(10000, "korean-hangul-formal", vcx), "일만");
    // Наследный синоним.
    assert_eq!(repr(11, "cjk-ideographic", vcx), "十一");
}
