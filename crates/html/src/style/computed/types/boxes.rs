//! Типы коробки: логические свойства и стороны, outline, position, anchor, overflow, порядок сторон, text-box-edge, border-shape.

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
    pub padding: LogicalSides,
    pub margin: LogicalSides,
    pub inset: LogicalSides,
    /// Логические кромки: сырые значения `border-block/inline-start/end` —
    /// раскладываются по физическим сторонам ПОСЛЕ наследования письма
    /// (у `vertical-rl` inline-start это ВЕРХ, а разбор писал влево).
    /// Порядок: [block-start, inline-end, block-end, inline-start].
    pub border: [Option<String>; 4],
}

/// Четыре логические стороны коробки.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogicalSides {
    pub inline_start: Option<Len>,
    pub inline_end: Option<Len>,
    pub block_start: Option<Len>,
    pub block_end: Option<Len>,
    /// Порядковый номер объявления каждой стороны (inline_start, inline_end,
    /// block_start, block_end) — спор с физической стороной решает более
    /// позднее объявление, а на какую физическую сторону ляжет логическая,
    /// известно только после наследования письма.
    pub seq: [u32; 4],
}

/// `outline`: рамка ВНЕ коробки, не влияющая на раскладку.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outline {
    pub width: Option<Len>,
    pub color: Option<Color>,
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

/// Порядковые номера объявлений физических сторон (top, right, bottom, left)
/// для полей, отступов и краёв — см. `LogicalSides::seq`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SideSeq {
    pub padding: [u32; 4],
    pub margin: [u32; 4],
    pub inset: [u32; 4],
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
