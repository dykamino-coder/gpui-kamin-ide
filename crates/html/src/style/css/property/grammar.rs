//! Грамматики длинных свойств реестра: по ней значение проверяется тем же
//! разбором, что и у применения.

/// Грамматика значения длинного свойства; проверка — тем же разбором, что
/// у применения (`Computed::accepts_longhand`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grammar {
    /// `<length-percentage> | auto | <anchor-size()>` (css-box-4 §3.1).
    Margin,
    /// `<length-percentage [0,∞]>` (css-box-4 §4.1).
    Padding,
    /// `auto | <length-percentage> | <anchor()> | <anchor-size()>`
    /// (css-position-3 §3.1, css-anchor-position-1 §4).
    Inset,
    /// Размеры коробки (css-sizing-3 §3.1-3.2, css-sizing-4, css-values-5
    /// `calc-size()`).
    Size,
    /// `<line-width>` (css-backgrounds-3 §4.3): `thin | medium | thick |
    /// <length [0,∞]>`; у `outline-width` та же (css-ui-4 §outline-width).
    LineWidth,
    /// `<line-style>` (css-backgrounds-3 §4.2).
    LineStyle,
    /// `<color>`, в том числе `currentcolor` (css-backgrounds-3 §4.1).
    LineColor,
    /// `auto | <outline-line-style>`: `<line-style>` без `hidden` (css-ui-4
    /// §outline-style).
    OutlineStyle,
    /// `auto | <color>` (css-ui-4 §outline-color).
    OutlineColor,
    /// `<length>` (css-ui-4 §outline-offset) и `inset` (WPT
    /// `outline-offset-inset-*`).
    OutlineOffset,
    /// Части `border-image` (css-backgrounds-3 §5): `-source`, `-slice`,
    /// `-width`, `-outset`, `-repeat`.
    BorderImage,
    /// Свойства фона (css-backgrounds-3 §3, css-backgrounds-4 §3.6): у
    /// каждого своя грамматика слоя, значение — список слоёв, у
    /// `background-color` — один цвет.
    Background,
    /// Слои маски (css-masking-1 §7): список слоёв по свойству.
    Mask,
    /// Части `mask-border` (css-masking-1 §8).
    MaskBorder,
    /// Свойства шрифта (css-fonts-4 §2-§7) и `line-height` (css-inline-3
    /// §line-height): вычисляются в фазе шрифта.
    Font,
    /// Пробелы, перенос и знаки акцента (css-text-4 §3, §5.1;
    /// css-text-decor-4 §3).
    Text,
    /// Линии оформления текста (css-text-decor-4 §2).
    TextDecor,
    /// Гибкие элементы и контейнер (css-flexbox-1 §5, §7).
    Flex,
    /// Выравнивание (css-align-3 §5-§6): у каждого свойства свой набор слов.
    Align,
    /// Промежутки (css-gaps-1 §2, css-align-3 §8).
    Gap,
    /// Шаблоны, неявные дорожки, автопоток и размещение сетки (css-grid-2
    /// §7-§8).
    Grid,
    /// Многоколонник (css-multicol-1 §3, css-multicol-2).
    Multicol,
    /// Линейки промежутков (css-gaps-1 §3-§6).
    GapRule,
}
