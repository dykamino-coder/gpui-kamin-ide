//! Своя строчная раскладка: разбиение абзаца на строки по правилам CSS.
//!
//! Переносчик GPUI знает одно правило — рвать по границам слов — и обойти его
//! снаружи можно лишь подсказками (нулевой пробел, словосоединитель). Этого
//! хватает на простые случаи и НЕ хватает на те, где ширина и точка разрыва
//! связаны:
//!
//! * `white-space: pre-wrap` — пробел в конце строки ВИСИТ за краем: место
//!   занимает, а перенос не вызывает;
//! * `white-space: break-spaces` — тот же пробел место занимает И даёт точку
//!   разрыва после себя;
//! * `overflow-wrap: break-word` — слово рвётся ТОЛЬКО если иначе не влезает;
//! * `line-break: anywhere` — разрыв где угодно, поверх запретов типографики;
//! * двунаправленный текст — знаки набираются в логическом порядке, а на экран
//!   идут в видимом, причём переставлять надо ГОТОВЫЕ прогоны, иначе рвётся
//!   арабская вязь.
//!
//! Поэтому строки считаются здесь: один раз меряется вся строка (`layout_line`
//! даёт положение каждого знака), по мере накопления ширины выбираются точки
//! разрыва, а на отрисовке каждая строка набирается своим `shape_line` и
//! рисуется на своём месте.

#[cfg(test)]
mod break_spaces_tests {
    //! Break spaces tests for paragraph.

    use gpui::{SharedString, px};

    use super::*;

    fn wrap() -> Wrap {
        Wrap {
            break_spaces: true,
            keep_spaces: true,
            ..Default::default()
        }
    }

    /// `white-space: break-spaces` даёт точку разрыва ПОСЛЕ каждого пробела.
    /// Пока их не было, строка рвалась только по правилам UAX-14, и
    /// сохранённый пробел уходил в конец строки вместо начала следующей.
    #[test]
    fn every_preserved_space_gives_a_stop() {
        let para = Paragraph::new(
            SharedString::from("X XX X".to_string()),
            vec![],
            px(25.),
            px(25.),
            Align::Left,
            wrap(),
        );
        let stops: Vec<usize> = para.opportunities().iter().map(|s| s.at).collect();
        assert!(stops.contains(&2), "после первого пробела: {stops:?}");
        assert!(stops.contains(&5), "после второго пробела: {stops:?}");
    }
}
mod line_types;
mod model;
#[cfg(test)]
mod tests {
    //! Tests for paragraph.

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
}
pub use crate::text::paragraph::line_types::Align;
use crate::text::paragraph::line_types::ELLIPSIS;
pub use crate::text::paragraph::line_types::Indent;
pub(crate) use crate::text::paragraph::line_types::Line;
use crate::text::paragraph::line_types::SOFT_HYPHEN;
pub use crate::text::paragraph::line_types::Wrap;
pub use crate::text::paragraph::model::Paragraph;

pub mod tabs;

mod atom_fit;
mod atom_placement;
mod content_baselines;
mod controlled_shape;
mod decor;
mod emphasis;
mod hyphen_shape;
mod overflow_marker;
mod ruby_justification;
mod ruby_overhang;
mod scroll_overflow;
mod selection_geometry;
mod spacing;
mod text_raster_origin;
mod vertical_content_baselines;
mod vertical_geometry;
mod vertical_inline;

mod element;
pub(crate) mod probes;
pub use crate::text::paragraph::probes::*;
pub(super) mod measure;
pub use crate::text::paragraph::measure::*;
pub(super) mod breaking;
pub use crate::text::paragraph::breaking::*;
pub(crate) mod justify;
pub(crate) use crate::text::paragraph::justify::*;
pub(super) mod runs;
use crate::text::paragraph::runs::*;
pub(super) mod geometry;
pub use crate::text::paragraph::geometry::*;
mod atoms;
mod clamp;
mod first_line;
mod fit;
pub(super) mod paint;
use crate::text::paragraph::paint::*;
