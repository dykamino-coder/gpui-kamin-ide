//! Тесты представлений предопределённых стилей счётчиков без раскладки: примеры
//! спецификации и расширенный диапазон восточноазиатских стилей
//! (CSS Counter Styles 3 §7.1.5) по эталонам WPT.

use super::{builtin, builtin_initial, builtin_repr};

#[test]
fn cjk_groups_preserve_zeros_and_language_markers() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for (style, value, expected) in [
        ("japanese-informal", 10_001, "一万一"),
        ("japanese-informal", 11_000, "一万千"),
        ("japanese-informal", 1_000_000_000_000_000, "一千兆"),
        ("japanese-formal", 12_345, "壱萬弐阡参百四拾伍"),
        ("korean-hangul-formal", 100_010_000, "일억 일만"),
        ("korean-hanja-informal", 10_000, "萬"),
        ("korean-hanja-informal", 100_010_000, "一億 萬"),
        ("korean-hanja-formal", 10_001, "壹萬 壹"),
        ("simp-chinese-informal", 100_010_000, "一亿零一万"),
        ("simp-chinese-informal", 100_100_000, "一亿零十万"),
        (
            "simp-chinese-informal",
            1_001_001_001_001,
            "一万亿零十亿零一百万一千零一",
        ),
        ("simp-chinese-formal", 10_010, "壹万零壹拾"),
        ("trad-chinese-informal", 100_000_001, "一億零一"),
        ("trad-chinese-formal", 1_000_000_000_001, "壹兆零壹"),
    ] {
        assert_eq!(builtin_repr(value, style, vcx), expected, "{style} {value}");
        assert_eq!(builtin_initial(style, value as u64).unwrap(), expected);
    }
}

#[test]
fn cjk_extended_range_and_negative_descriptors() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for style in [
        "japanese-informal",
        "korean-hanja-formal",
        "simp-chinese-formal",
    ] {
        let data = builtin(style).unwrap();
        assert_eq!(
            data.range,
            Some((-9_999_999_999_999_999, 9_999_999_999_999_999))
        );
        let body = builtin_repr(1_000_000_000_001, style, vcx);
        assert_eq!(
            builtin_repr(-1_000_000_000_001, style, vcx),
            format!("{}{body}", data.negative.0)
        );
        assert_eq!(
            builtin_repr(10_000_000_000_000_000, style, vcx),
            "一〇〇〇〇〇〇〇〇〇〇〇〇〇〇〇〇"
        );
        assert_eq!(
            builtin_repr(-10_000_000_000_000_000, style, vcx),
            "-10000000000000000"
        );
    }
}

#[test]
fn full_width_values_survive_numeric_and_fallback_styles() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(
        builtin_repr(i64::MIN, "decimal", vcx),
        "-9223372036854775808"
    );
    assert_eq!(
        builtin_repr(i64::MIN, "arabic-indic", vcx),
        "-٩٢٢٣٣٧٢٠٣٦٨٥٤٧٧٥٨٠٨"
    );
    assert_eq!(
        builtin_repr(10_000_000_000_000_000, "lower-roman", vcx),
        "10000000000000000"
    );
    assert_eq!(builtin_repr(10_999, "hebrew", vcx), "י׳תתקצט");
    assert_eq!(builtin_repr(11_000, "hebrew", vcx), "11000");
}
