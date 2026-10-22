//! Типы коробки: логические свойства и стороны, outline, position, anchor, overflow, порядок
//! сторон, text-box-edge, border-shape.

use super::*;

/// Стороны и размеры, заданные ЛОГИЧЕСКИ.
///
/// Какая сторона логического начала физическая — знает только письмо, а оно
/// наследуется. Поэтому значения доживают до отдельного прохода по дереву
/// (`doc::resolve_logical`) и лишь там ложатся на физические поля. Перевод
/// при разборе, как было раньше, молча считал письмо горизонтальным: в
/// вертикальном тексте `margin-block` разворачивал отступ не по той оси.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Logical {
    pub inline_size: Option<Len>,
    pub block_size: Option<Len>,
    /// `contain-intrinsic-inline-size` / `-block-size`: подменная величина
    /// по логическим осям — раскладывается по физическим после наследования
    /// письма, как и остальные логические свойства.
    pub ci_inline: Option<f32>,
    pub ci_block: Option<f32>,
    pub min_inline: Option<Len>,
    pub min_block: Option<Len>,
    pub max_inline: Option<Len>,
    pub max_block: Option<Len>,
    /// Порядок каскада логических размеров (`inline, block, min-inline,
    /// min-block, max-inline, max-block`): спор с физическими решает
    /// более сильное объявление.
    pub size_order: [Option<DeclOrder>; 6],
    /// `inherit` логического размера: ложится признаком наследования на
    /// физический размер, выбранный письмом.
    pub size_inherit: [bool; 6],
    pub padding: LogicalSides,
    pub margin: LogicalSides,
    pub inset: LogicalSides,
    /// Логические кромки `border-{inline,block}-{start,end}-{width,style,
    /// color}` (css-logical-1 §4.4): `[часть][сторона]` — часть 0 толщина,
    /// 1 стиль, 2 цвет; сторона как у [`LogicalSides`]. Значение проверено
    /// разбором и ложится на физическую сторону ПОСЛЕ наследования письма
    /// вместе с местом объявления в каскаде.
    pub border: [[Option<(DeclOrder, String)>; 4]; 3],
}

/// Четыре логические стороны коробки.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogicalSides {
    pub inline_start: Option<Len>,
    pub inline_end: Option<Len>,
    pub block_start: Option<Len>,
    pub block_end: Option<Len>,
    /// Порядок каскада каждой стороны (inline_start, inline_end,
    /// block_start, block_end): спор с физической стороной решает более
    /// сильное объявление (css-logical-1 §4), а на какую физическую сторону
    /// ляжет логическая, известно только после наследования письма.
    pub order: [Option<DeclOrder>; 4],
    /// `inherit` стороны: ложится признаком наследования на физическую.
    pub inherit: [bool; 4],
}

impl LogicalSides {
    /// Значение стороны по индексу (inline_start, inline_end, block_start,
    /// block_end).
    pub fn get(&self, side: usize) -> Option<Len> {
        [
            self.inline_start,
            self.inline_end,
            self.block_start,
            self.block_end,
        ][side]
    }

    pub fn slot(&mut self, side: usize) -> &mut Option<Len> {
        match side {
            0 => &mut self.inline_start,
            1 => &mut self.inline_end,
            2 => &mut self.block_start,
            _ => &mut self.block_end,
        }
    }
}

/// `outline`: рамка ВНЕ коробки, не влияющая на раскладку.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outline {
    pub width: Option<Len>,
    /// `outline-color` (css-ui-4 §outline-color): `auto` — акцентный цвет при
    /// `outline-style: auto`, иначе `currentcolor` (`Computed::used_outline_color`).
    pub color: crate::style::values::value::ColorOrAuto,
    pub offset: Option<Len>,
    /// `outline-offset: inset` (css-ui-4; WPT `outline-offset-inset-*`):
    /// сдвиг равен минус толщине. Толщина известна только к отрисовке,
    /// поэтому здесь метка, а не длина.
    pub inset: bool,
    /// 0 — none/hidden, 1 — solid/groove/…, 2 — auto, 3 — dotted,
    /// 4 — dashed, 5 — double (two rings with a transparent gap).
    pub style: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Position {
    Static,
    Relative,
    Absolute,
    /// `fixed` — отсчёт от окна, прокрутка его не двигает.
    Fixed,
    /// `sticky` — в потоке, пока лента не увезла элемент за край; дальше он
    /// стоит у края видимой части, но не покидает родителя.
    Sticky,
}

/// `position-anchor` (css-anchor-position-1 §position-anchor): якорь по
/// умолчанию для `anchor()` без имени. `normal` без `position-area` ведёт
/// себя как `none`; `match-parent` пока не решается (нет пар).
#[derive(Clone, Debug, PartialEq)]
pub enum PositionAnchor {
    Normal,
    None,
    Auto,
    Named(String),
    MatchParent,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
    /// Обрезка БЕЗ скролл-контейнера: авто-минимум flex/grid-элемента
    /// остаётся по содержимому (CSSWG #7714; min-size-auto-overflow-clip).
    Clip,
}

/// Место применённого объявления в каскаде: номер прохода каскада по стилю
/// (слой `:hover` или кадр поверх готового стиля — более поздний проход) и
/// ключ каскада победителя (css-cascade-5 §6.2). Сравнивается целиком.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclOrder {
    pub(crate) pass: u32,
    pub(crate) key: crate::style::cascade::key::CascadeKey,
}

/// Порядок каскада физических сторон (top, right, bottom, left) полей,
/// отступов и вставок и физических размеров (`width, height, min-width,
/// min-height, max-width, max-height`) — см. `LogicalSides::order`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SideOrder {
    pub(crate) padding: [DeclOrder; 4],
    pub(crate) margin: [DeclOrder; 4],
    pub(crate) inset: [DeclOrder; 4],
    pub(crate) size: [DeclOrder; 6],
    /// Рамка: `[часть][сторона]`, часть 0 толщина, 1 стиль, 2 цвет.
    pub(crate) border: [[DeclOrder; 4]; 3],
}

/// Метрика края текста (`<text-edge>`, css-inline-3 §4.3).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TextEdge {
    /// Подъём/спуск шрифта — начальное значение.
    #[default]
    Text,
    /// Высота прописной (только верхний край).
    Cap,
    /// Высота строчной (только верхний край).
    Ex,
    /// Алфавитная базовая линия (только нижний край).
    Alphabetic,
    /// Идеографические базовые линии (края em-коробки, css-inline-3
    /// §text-edges); без таблицы `BASE` — синтез из подъёма и спуска.
    Ideographic,
    /// Идеографические линии чернил; без таблицы — как `Ideographic`.
    IdeographicInk,
}

/// `border-shape` (css-borders-4 §border-shape): одна фигура — рамка
/// обводкой по её контуру толщиной «relevant side»; две — заливка между
/// внешней и внутренней. Текст фигуры хранится как есть, доли резолвит
/// отрисовка от опорной коробки (`geometry-box`: 0 border, 1 margin,
/// 2 padding, 3 content, 4 half-border-box).
#[derive(Clone, Debug, PartialEq)]
pub struct BorderShape {
    pub outer: String,
    pub outer_box: u8,
    pub inner: Option<(String, u8)>,
}
