//! Tests for paragraph; split out to keep the owning module within 250 lines.

use crate::text::paragraph::justify::*;
use gpui::{SharedString, px};

use super::*;

/// Абзац без набора: точки разрыва считаются по тексту и правилам, а
/// система шрифтов для этого не нужна.
fn para(text: &str, wrap: Wrap) -> Paragraph {
    Paragraph::new(
        SharedString::from(text.to_string()),
        vec![],
        px(16.),
        px(16.),
        Align::Left,
        wrap,
    )
}

fn stops(text: &str, wrap: Wrap) -> Vec<usize> {
    para(text, wrap)
        .opportunities()
        .iter()
        .map(|s| s.at)
        .collect()
}

#[test]
fn a_hard_break_is_a_stop_even_without_wrapping() {
    let wrap = Wrap {
        nowrap: true,
        ..Default::default()
    };
    assert_eq!(stops("a\nb", wrap), vec![2]);
}

#[test]
fn break_spaces_stops_after_every_kept_space() {
    let wrap = Wrap {
        break_spaces: true,
        ..Default::default()
    };
    // Пробел даёт точку разрыва ПОСЛЕ себя — и первый, и второй.
    assert_eq!(stops("a  b", wrap), vec![2, 3]);
}

#[test]
fn break_all_stops_between_letters_but_not_before_a_space() {
    let wrap = Wrap {
        break_all: true,
        ..Default::default()
    };
    // Между буквами — точка, перед пробелом — нет: рвать там нечего,
    // пробел и так свисает за край. После пробела точка от типографики.
    assert_eq!(stops("ab c", wrap), vec![1, 3]);
}

#[test]
fn anywhere_stops_before_every_character() {
    let wrap = Wrap {
        anywhere: true,
        ..Default::default()
    };
    assert_eq!(stops("a b", wrap), vec![1, 2]);
}

#[test]
fn keep_all_keeps_ideographs_together() {
    let wrap = Wrap {
        keep_all: true,
        ..Default::default()
    };
    // Между иероглифами разрыва нет, около пробела — есть.
    assert_eq!(stops("中文 中文", wrap), vec![7]);
}

#[test]
fn hanging_spaces_are_cut_off_the_measured_part() {
    assert_eq!(trim_hanging("ab  "), 2);
    assert_eq!(trim_hanging("ab"), 2);
    assert_eq!(trim_hanging("  "), 0);
}

/// Распорка строчной коробки не должна съедать точку переноса ПЕРЕД собой:
/// её класс по UAX-14 (WJ) запрещает разрыв с обеих сторон, и пробел
/// перед `<span>` с отступом переставал быть точкой переноса.
#[test]
fn spacer_keeps_the_break_before_it() {
    let text = "aaa \u{feff}bbb";
    let mut para = para(text, Wrap::default());
    assert!(
        !para.opportunities().iter().any(|s| s.at == 4),
        "пока распорка не объявлена, разрыв по пробелу запрещён"
    );
    para.spacers = vec![4];
    let stops: Vec<usize> = para.opportunities().iter().map(|s| s.at).collect();
    // Разрыв встаёт НА распорку: поле коробки уходит на новую строку
    // вместе со своим текстом.
    assert_eq!(stops, vec![4]);
}

/// Отступ первой строки: кому он достаётся при `each-line` и `hanging`.
#[test]
fn indent_goes_to_the_right_lines() {
    let mut para = para("a", Wrap::default());
    let of = |p: &Paragraph, head, first| f32::from(p.indent_of(head, first, None));
    para.indent = Indent {
        px: 40.,
        ..Default::default()
    };
    assert_eq!(of(&para, true, true), 40., "первая строка блока");
    assert_eq!(of(&para, true, false), 0., "первая строка ВТОРОГО куска");
    assert_eq!(of(&para, false, true), 0., "перенесённая строка");
    para.indent.each_line = true;
    assert_eq!(of(&para, true, false), 40., "each-line: каждый кусок");
    assert_eq!(of(&para, false, true), 0., "each-line: не перенос");
    para.indent = Indent {
        px: 40.,
        hanging: true,
        ..Default::default()
    };
    assert_eq!(of(&para, true, true), 0., "hanging: кроме первой");
    assert_eq!(of(&para, false, true), 40.);
    assert_eq!(of(&para, true, false), 40.);
}

/// Доля берётся от ширины строки, а при замере по содержимому её нет.
#[test]
fn indent_share_needs_a_limit() {
    let mut para = para("a", Wrap::default());
    para.indent = Indent {
        pct: 0.1,
        ..Default::default()
    };
    assert_eq!(f32::from(para.indent_of(true, true, Some(px(300.)))), 30.);
    assert_eq!(f32::from(para.indent_of(true, true, None)), 0.);
}
