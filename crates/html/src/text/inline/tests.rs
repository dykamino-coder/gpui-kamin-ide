//! Tests for inline; split out to keep the owning module within 250 lines.

use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline::hyphenate::*;
use crate::text::inline::runs::*;
use crate::text::inline::whitespace::*;

use super::*;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::TextAlign;
use crate::style::css::parse_decls;

fn styled(css: &str) -> Computed {
    let mut c = Computed::default();
    c.apply_decls(&parse_decls(css));
    c
}

#[test]
fn spaces_collapse_like_html() {
    assert_eq!(normalize_spaces("  два\n\tслова  "), " два слова ");
    assert_eq!(normalize_spaces("a\n\nb"), "a b");
    // Идеографический и неразрывный пробелы — знаки, а не пробелы CSS:
    // они остаются как есть и держат свою ширину.
    assert_eq!(normalize_spaces("あ\u{3000}あ"), "あ\u{3000}あ");
    assert_eq!(normalize_spaces("a\u{00a0}b"), "a\u{00a0}b");
}

#[test]
fn logical_alignment_follows_the_writing_direction() {
    // Наследование хранит ЛОГИЧЕСКОЕ значение, разворот — на отрисовке.
    // Иначе левый край, посчитанный для тела страницы, доставался бы по
    // наследству вложенному блоку справа налево.
    let parent = Computed::default();
    let ltr = inherit(&parent, &styled("text-align: start"));
    assert_eq!(ltr.text_align, Some(TextAlign::Start));
    assert_eq!(
        crate::text::paragraph::align_for(&ltr),
        crate::text::paragraph::Align::Left
    );
    let rtl = inherit(&parent, &styled("text-align: start; direction: rtl"));
    assert_eq!(
        crate::text::paragraph::align_for(&rtl),
        crate::text::paragraph::Align::Right
    );
    let rtl_end = inherit(&parent, &styled("text-align: end; direction: rtl"));
    assert_eq!(
        crate::text::paragraph::align_for(&rtl_end),
        crate::text::paragraph::Align::Left
    );
    // Умолчание CSS — `start`: без выключки текст справа налево прижат
    // вправо, а не влево.
    let bare = inherit(&parent, &styled("direction: rtl"));
    assert_eq!(
        crate::text::paragraph::align_for(&bare),
        crate::text::paragraph::Align::Right
    );
    // Наследник блока справа налево берёт сторону письма у него.
    let child = inherit(&bare, &Computed::default());
    assert_eq!(
        crate::text::paragraph::align_for(&child),
        crate::text::paragraph::Align::Right
    );
}

#[test]
fn breaking_rules_leave_the_text_alone_for_the_own_layout() {
    // Подсказки переносчику GPUI ставятся только там, где абзац рисует
    // сам движок. Своей раскладке они бы мешали: невидимый знак стал бы
    // лишней точкой разрыва (правила считает `lines::Paragraph`).
    for css in [
        "line-break: anywhere",
        "word-break: break-all",
        "word-break: keep-all",
        "white-space: break-spaces",
    ] {
        let style = styled(css);
        assert!(crate::text::paragraph::rules(&style).is_some(), "{css}");
        assert_eq!(breakable("a b", &style), "a b", "{css}");
    }
    // Своей раскладке мягкий перенос доезжает КАК ЕСТЬ: она знает его
    // точкой разрыва и рисует на его месте знак переноса.
    assert_eq!(breakable("a\u{ad}b", &Computed::default()), "a\u{ad}b");
    assert_eq!(breakable("a\u{ad}b", &styled("hyphens: none")), "ab");
}

#[test]
fn plain_word_break_is_left_alone() {
    // Умолчание не трогается: там перенос между знаками разрешён самим
    // переносчиком, вставлять нечего.
    assert_eq!(breakable("中文", &styled("word-break: normal")), "中文");
}

#[test]
fn same_size_pieces_go_into_one_block() {
    let pieces = vec![
        Piece::Text {
            text: "обычный ".into(),
            style: styled(""),
        },
        Piece::Text {
            text: "жирный".into(),
            style: styled("font-weight: 700"),
        },
    ];
    assert!(single_block(&pieces, 13.0), "вес не мешает единому блоку");
}

#[test]
fn different_size_stays_in_one_block() {
    let pieces = vec![
        Piece::Text {
            text: "обычный ".into(),
            style: styled(""),
        },
        Piece::Text {
            text: "крупный".into(),
            style: styled("font-size: 24px"),
        },
    ];
    // Кегль едет в прогон (патч GPUI), поэтому разный размер больше не
    // выгоняет абзац в запасную ветку из отдельных слов: раньше там
    // строки наезжали друг на друга.
    assert!(
        single_block(&pieces, 13.0),
        "иной размер собирается единым блоком"
    );
    assert_eq!(
        max_font_size(&pieces, 13.0, 13.0),
        24.0,
        "строка растёт под кусок"
    );
}

#[test]
fn inheritance_carries_text_not_box() {
    let parent = styled("color: #ff0000; padding: 10px; font-size: 20px");
    let child = styled("font-weight: 700");
    let merged = inherit(&parent, &child);
    assert_eq!(merged.color.map(|c| c.r), Some(1.0), "цвет наследуется");
    assert_eq!(merged.font_size, Some(Len::Px(20.0)), "размер наследуется");
    assert_eq!(merged.font_weight, Some(700), "свой вес сохранён");
    assert_eq!(merged.padding.top, None, "отступ родителя вниз не идёт");
}

#[test]
fn own_style_wins_over_inherited() {
    let parent = styled("color: #ff0000");
    let child = styled("color: #0000ff");
    assert_eq!(inherit(&parent, &child).color.map(|c| c.b), Some(1.0));
}
