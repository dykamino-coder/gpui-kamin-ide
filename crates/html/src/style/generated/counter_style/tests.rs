//! Тесты стилей счётчиков (style::generated::counter_style): встроенные системы и представления.

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
