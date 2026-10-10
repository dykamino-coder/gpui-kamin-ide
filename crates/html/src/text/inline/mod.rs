//! Инлайн-поток: текст с вкраплениями `<b>`, `<a>`, `<code>` внутри абзаца.
//!
//! Здесь лежит главное расхождение GPUI с HTML. В браузере абзац — это поток
//! строк, куда встраиваются куски любого размера, и строка растёт под самый
//! высокий из них. В GPUI текстовый блок — лист раскладки: размер шрифта у него
//! ОДИН на весь блок (`shape_text` принимает `font_size` скаляром), и метрики
//! строки тоже одни.
//!
//! Отсюда две стратегии, и выбор между ними делается по факту содержимого:
//!
//! * **Один блок текста.** Если все куски абзаца одного размера и ни один не
//!   несёт собственного бокса (фон, рамка, отступы), они собираются в один
//!   `StyledText` с прогонами. Перенос строк тогда честный — как в браузере,
//!   по словам, сквозь границы `<b>` и `<a>`.
//! * **Строка из элементов.** Иначе куски становятся отдельными элементами в
//!   гибкой строке с переносом. Перенос идёт по кускам, а не по словам внутри
//!   них — это заметно на длинном `<code>`, но зато размеры и боксы честные.
//!
//! Первая ветка покрывает подавляющее большинство: жирный, курсив, ссылка,
//! цвет. Вторая включается там, где без неё пришлось бы врать про размер.

mod collapse;
mod containing_block;
#[cfg(test)]
mod em_tests;
mod first_line;
#[cfg(test)]
mod tests;
pub use crate::text::inline::collapse::collapse_across_pieces;
pub(crate) use crate::text::inline::containing_block::backdrop_root;
pub(crate) use crate::text::inline::containing_block::establishes_cb;
use crate::text::inline::containing_block::mark_inline_cb;
pub(crate) use crate::text::inline::containing_block::note_abs_cb;
pub(crate) use crate::text::inline::containing_block::set_atom_cb;
pub(crate) use crate::text::inline::containing_block::take_abs_cb;
pub(crate) use crate::text::inline::containing_block::take_atom_cb;
pub use crate::text::inline::first_line::style_first_line;

mod emphasis;
pub use crate::text::inline::emphasis::emphasis_spans;

mod empty_inline;
mod first_letter;
mod first_line_background;
pub use crate::text::inline::first_letter::split_first_letter;

pub(crate) mod bidi_controls;
mod inline_spacing;
mod lang_case;
mod physical_projection;
pub(crate) mod physical_sides;
mod tabs;
pub use crate::text::inline::bidi_controls::bidi_marks;
pub use crate::text::inline::tabs::tab_stops;

mod text_case;

use crate::dom::{Element, Node};
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement};
pub(super) mod collect;
use crate::text::inline::collect::*;
pub(super) mod hyphenate;
pub use crate::text::inline::hyphenate::*;
pub(super) mod spans;
pub use crate::text::inline::spans::*;
pub(super) mod spacers;
pub use crate::text::inline::spacers::*;
pub(super) mod whitespace;
pub use crate::text::inline::whitespace::*;
pub(super) mod case;
pub use crate::text::inline::case::*;
pub(super) mod runs;
pub use crate::text::inline::runs::*;

/// Кусок инлайн-содержимого: либо текст со своим стилем, либо готовый элемент
/// (картинка, кнопка — то, что текстом не является).
pub enum Piece {
    Text {
        text: String,
        style: Computed,
    },
    Atom(AnyElement),
    /// Элемент ВНЕ потока строки: места не занимает, но рисуется там, где
    /// стоит в тексте (абсолютный элемент на статической позиции). В отличие
    /// от `Atom` не выводит абзац из текстового пути — иначе строка теряет
    /// пробелы и перенос по словам (`line-breaking-018`).
    ///
    /// Второе поле — БЛОЧНЫЙ уровень гипотетической коробки: её статическая
    /// позиция — начало СЛЕДУЮЩЕЙ строки, а не точка в текущей (CSS 2.1
    /// §10.6.4/§10.3.7 «if position had been static»; Blink
    /// `LogicalStaticPosition` блочного OOF в строчном контексте — блок-
    /// начало после текущей строки, строчное начало — край содержимого).
    Overlay(AnyElement, OverlayAt),
}

/// Как кусок вне потока садится на своё место в тексте (`lines.rs`).
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct OverlayAt {
    /// Гипотетическая коробка БЛОЧНАЯ: место — начало следующей строки.
    pub next_line: bool,
    /// Относительный сдвиг строчных предков по ДО-ПОВОРОТНОЙ оси y в
    /// повёрнутом абзаце. Прикладывается в `lines.rs` вместе с округлением
    /// до целой физической точки ВНИЗ — так же, как глиф соседнего текста
    /// (`window.rs: paint_glyph` берёт `floor`, раскладка — `round`).
    pub rot_dy: f32,
    /// То же по до-поворотной x (строчная ось). Отбивкой его не задать:
    /// при `direction: rtl` `inset-inline-start` даёт ОТРИЦАТЕЛЬНЫЙ сдвиг, а
    /// отрицательная отбивка обнуляется — коробка оставалась на месте, текст
    /// уезжал на 2px, и полоса красного проступала (`static-position/
    /// v{lr,rl}-rtl-*`, `cb`-случаи).
    pub rot_dx: f32,
    /// Строчный абсолют на статической точке повёрнутого rtl-абзаца: сторону,
    /// которой коробка висит на точке, решает УРОВЕНЬ bidi в точке
    /// (`lines.rs: rtl_level_at`), а не направление блока. Гипотетическая
    /// коробка стоит В ПРОГОНЕ: в ltr-прогоне (латиница, Ahem) она уходит от
    /// точки вправо, и строчное начало rtl-блока — её правый край — лежит на
    /// ширину правее точки (CSS 2.1 §10.3.7 «set 'right' to the static
    /// position»: статическая позиция — край гипотетической коробки).
    pub bidi_hang: bool,
    /// Абсолют с краями по ОБЕИМ осям, чей содержащий блок — позиционированный
    /// строчный предок в этом же абзаце (`render.rs: atom_element_raw`).
    pub edges: bool,
    /// Содержащий блок такого абсолюта — строчная коробка (`mark_inline_cb`).
    pub cb: Option<InlineCb>,
    /// Пустой кусок-метка края содержимого такой коробки `(id, начало?)`:
    /// байтовые края считаются по ГОТОВЫМ кускам (`overlays`) — схлопывание
    /// пробелов на границах кусков идёт уже после сбора.
    pub cb_marker: Option<(u32, bool)>,
}

/// Содержащий блок из фрагментов строчной коробки (CSS 2.1 §10.1 п.4.1;
/// Blink `out_of_flow_layout_part.cc` `ComputeInlineContainingBlocks`:
/// начало — верхний строчно-начальный угол первого фрагмента, конец —
/// нижний строчно-конечный угол последнего, отрицательный размер — ноль).
/// Байтовые края СОДЕРЖИМОГО коробки — относительно места самого куска.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InlineCb {
    /// Метка коробки (`OverlayAt::cb_marker`).
    pub id: u32,
    pub start: isize,
    pub end: isize,
    /// Отбивка коробки `[top, right, bottom, left]`: содержащий блок — край
    /// отбивки (§10.1 п.4: «padding edges»).
    pub pad: [f32; 4],
    /// Относительный сдвиг коробки и её строчных предков (§9.4.3).
    pub shift: (f32, f32),
}

/// Собрать инлайн-куски из детей узла.
pub fn collect(
    children: &[Node],
    inherited: &Computed,
    atom: &mut dyn FnMut(&Element) -> Option<Piece>,
) -> Vec<Piece> {
    collect_with_empty_metrics(
        children,
        inherited,
        atom,
        empty_inline::has_text(children),
        &mut text_case::Context::default(),
    )
}

use crate::text::inline::containing_block::INLINE_CB_DEPTH;

use crate::text::inline::containing_block::ATOM_CB;
