//! Таблица длинных свойств реестра: имя, начальное значение (css-box-4,
//! css-position-3, css-sizing-3, css-logical-1, css-backgrounds-3/4,
//! css-masking-1, css-ui-4),
//! наследование, грамматика и место в логической группе (css-logical-1 §4:
//! логическое и физическое свойство одной стороны делят вычисленное
//! значение).

use crate::document::state::ValueCx;

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
}

/// Логическая группа (Stylo `logical_group`): физические и логические
/// свойства одной группы делят поля стиля.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxGroup {
    Margin,
    Padding,
    Inset,
    Size,
    BorderWidth,
    BorderStyle,
    BorderColor,
}

/// Место свойства в группе. Стороны: физические `top, right, bottom,
/// left`, логические `inline-start, inline-end, block-start, block-end`.
/// Размеры: физические `width, height, min-width, min-height, max-width,
/// max-height`, логические `inline, block, min-inline, min-block,
/// max-inline, max-block`. Свойство вне логических групп (`outline-*`,
/// части `border-image`) — `Single`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Physical(BoxGroup, usize),
    Logical(BoxGroup, usize),
    Single,
}

macro_rules! longhands {
    ($($id:ident = $name:literal, $initial:literal, $inherited:literal, $grammar:ident, $slot:expr;)*) => {
        /// Длинное свойство реестра.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum LonghandId { $($id),* }

        impl LonghandId {
            /// Все длинные свойства в порядке таблицы.
            pub const ALL: &'static [LonghandId] = &[$(LonghandId::$id),*];
            const NAMES: &'static [&'static str] = &[$($name),*];
            const INITIAL: &'static [&'static str] = &[$($initial),*];
            const INHERITED: &'static [bool] = &[$($inherited),*];
            const GRAMMAR: &'static [Grammar] = &[$(Grammar::$grammar),*];
            const SLOT: &'static [Slot] = &[$($slot),*];

            /// Свойство реестра по имени (строчными, как его хранит блок).
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(LonghandId::$id),)*
                    _ => None,
                }
            }
        }
    };
}

use BoxGroup::{
    BorderColor as BC, BorderStyle as BS, BorderWidth as BW, Inset as I, Margin as M, Padding as P,
    Size as S,
};
use Slot::{Logical as L, Physical as F, Single as N};

longhands! {
    MarginTop = "margin-top", "0", false, Margin, F(M, 0);
    MarginRight = "margin-right", "0", false, Margin, F(M, 1);
    MarginBottom = "margin-bottom", "0", false, Margin, F(M, 2);
    MarginLeft = "margin-left", "0", false, Margin, F(M, 3);
    MarginInlineStart = "margin-inline-start", "0", false, Margin, L(M, 0);
    MarginInlineEnd = "margin-inline-end", "0", false, Margin, L(M, 1);
    MarginBlockStart = "margin-block-start", "0", false, Margin, L(M, 2);
    MarginBlockEnd = "margin-block-end", "0", false, Margin, L(M, 3);
    PaddingTop = "padding-top", "0", false, Padding, F(P, 0);
    PaddingRight = "padding-right", "0", false, Padding, F(P, 1);
    PaddingBottom = "padding-bottom", "0", false, Padding, F(P, 2);
    PaddingLeft = "padding-left", "0", false, Padding, F(P, 3);
    PaddingInlineStart = "padding-inline-start", "0", false, Padding, L(P, 0);
    PaddingInlineEnd = "padding-inline-end", "0", false, Padding, L(P, 1);
    PaddingBlockStart = "padding-block-start", "0", false, Padding, L(P, 2);
    PaddingBlockEnd = "padding-block-end", "0", false, Padding, L(P, 3);
    Top = "top", "auto", false, Inset, F(I, 0);
    Right = "right", "auto", false, Inset, F(I, 1);
    Bottom = "bottom", "auto", false, Inset, F(I, 2);
    Left = "left", "auto", false, Inset, F(I, 3);
    InsetInlineStart = "inset-inline-start", "auto", false, Inset, L(I, 0);
    InsetInlineEnd = "inset-inline-end", "auto", false, Inset, L(I, 1);
    InsetBlockStart = "inset-block-start", "auto", false, Inset, L(I, 2);
    InsetBlockEnd = "inset-block-end", "auto", false, Inset, L(I, 3);
    Width = "width", "auto", false, Size, F(S, 0);
    Height = "height", "auto", false, Size, F(S, 1);
    MinWidth = "min-width", "auto", false, Size, F(S, 2);
    MinHeight = "min-height", "auto", false, Size, F(S, 3);
    MaxWidth = "max-width", "none", false, Size, F(S, 4);
    MaxHeight = "max-height", "none", false, Size, F(S, 5);
    InlineSize = "inline-size", "auto", false, Size, L(S, 0);
    BlockSize = "block-size", "auto", false, Size, L(S, 1);
    MinInlineSize = "min-inline-size", "auto", false, Size, L(S, 2);
    MinBlockSize = "min-block-size", "auto", false, Size, L(S, 3);
    MaxInlineSize = "max-inline-size", "none", false, Size, L(S, 4);
    MaxBlockSize = "max-block-size", "none", false, Size, L(S, 5);
    BorderTopWidth = "border-top-width", "medium", false, LineWidth, F(BW, 0);
    BorderRightWidth = "border-right-width", "medium", false, LineWidth, F(BW, 1);
    BorderBottomWidth = "border-bottom-width", "medium", false, LineWidth, F(BW, 2);
    BorderLeftWidth = "border-left-width", "medium", false, LineWidth, F(BW, 3);
    BorderInlineStartWidth = "border-inline-start-width", "medium", false, LineWidth, L(BW, 0);
    BorderInlineEndWidth = "border-inline-end-width", "medium", false, LineWidth, L(BW, 1);
    BorderBlockStartWidth = "border-block-start-width", "medium", false, LineWidth, L(BW, 2);
    BorderBlockEndWidth = "border-block-end-width", "medium", false, LineWidth, L(BW, 3);
    BorderTopStyle = "border-top-style", "none", false, LineStyle, F(BS, 0);
    BorderRightStyle = "border-right-style", "none", false, LineStyle, F(BS, 1);
    BorderBottomStyle = "border-bottom-style", "none", false, LineStyle, F(BS, 2);
    BorderLeftStyle = "border-left-style", "none", false, LineStyle, F(BS, 3);
    BorderInlineStartStyle = "border-inline-start-style", "none", false, LineStyle, L(BS, 0);
    BorderInlineEndStyle = "border-inline-end-style", "none", false, LineStyle, L(BS, 1);
    BorderBlockStartStyle = "border-block-start-style", "none", false, LineStyle, L(BS, 2);
    BorderBlockEndStyle = "border-block-end-style", "none", false, LineStyle, L(BS, 3);
    BorderTopColor = "border-top-color", "currentcolor", false, LineColor, F(BC, 0);
    BorderRightColor = "border-right-color", "currentcolor", false, LineColor, F(BC, 1);
    BorderBottomColor = "border-bottom-color", "currentcolor", false, LineColor, F(BC, 2);
    BorderLeftColor = "border-left-color", "currentcolor", false, LineColor, F(BC, 3);
    BorderInlineStartColor = "border-inline-start-color", "currentcolor", false, LineColor, L(BC, 0);
    BorderInlineEndColor = "border-inline-end-color", "currentcolor", false, LineColor, L(BC, 1);
    BorderBlockStartColor = "border-block-start-color", "currentcolor", false, LineColor, L(BC, 2);
    BorderBlockEndColor = "border-block-end-color", "currentcolor", false, LineColor, L(BC, 3);
    OutlineWidth = "outline-width", "medium", false, LineWidth, N;
    OutlineStyle = "outline-style", "none", false, OutlineStyle, N;
    OutlineColor = "outline-color", "auto", false, OutlineColor, N;
    OutlineOffset = "outline-offset", "0", false, OutlineOffset, N;
    BorderImageSource = "border-image-source", "none", false, BorderImage, N;
    BorderImageSlice = "border-image-slice", "100%", false, BorderImage, N;
    BorderImageWidth = "border-image-width", "1", false, BorderImage, N;
    BorderImageOutset = "border-image-outset", "0", false, BorderImage, N;
    BorderImageRepeat = "border-image-repeat", "stretch", false, BorderImage, N;
    BackgroundColor = "background-color", "transparent", false, Background, N;
    BackgroundImage = "background-image", "none", false, Background, N;
    BackgroundPositionX = "background-position-x", "0%", false, Background, N;
    BackgroundPositionY = "background-position-y", "0%", false, Background, N;
    BackgroundSize = "background-size", "auto", false, Background, N;
    BackgroundRepeat = "background-repeat", "repeat", false, Background, N;
    BackgroundAttachment = "background-attachment", "scroll", false, Background, N;
    BackgroundOrigin = "background-origin", "padding-box", false, Background, N;
    BackgroundClip = "background-clip", "border-box", false, Background, N;
    MaskImage = "mask-image", "none", false, Mask, N;
    MaskMode = "mask-mode", "match-source", false, Mask, N;
    MaskPosition = "mask-position", "0% 0%", false, Mask, N;
    MaskSize = "mask-size", "auto", false, Mask, N;
    MaskRepeat = "mask-repeat", "repeat", false, Mask, N;
    MaskOrigin = "mask-origin", "border-box", false, Mask, N;
    MaskClip = "mask-clip", "border-box", false, Mask, N;
    MaskComposite = "mask-composite", "add", false, Mask, N;
    MaskBorderSource = "mask-border-source", "none", false, MaskBorder, N;
    MaskBorderSlice = "mask-border-slice", "0", false, MaskBorder, N;
    MaskBorderWidth = "mask-border-width", "auto", false, MaskBorder, N;
    MaskBorderOutset = "mask-border-outset", "0", false, MaskBorder, N;
    MaskBorderRepeat = "mask-border-repeat", "stretch", false, MaskBorder, N;
    MaskBorderMode = "mask-border-mode", "alpha", false, MaskBorder, N;
}

impl LonghandId {
    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    /// Начальное значение (css-cascade-5 §7.3.1) — текстом той же
    /// грамматики, что и объявление.
    pub fn initial(self) -> &'static str {
        Self::INITIAL[self as usize]
    }

    /// Наследуется ли по умолчанию: решает `unset` (§7.3.3).
    pub fn inherited(self) -> bool {
        Self::INHERITED[self as usize]
    }

    pub fn grammar(self) -> Grammar {
        Self::GRAMMAR[self as usize]
    }

    pub fn slot(self) -> Slot {
        Self::SLOT[self as usize]
    }

    /// Годится ли значение (не общее слово и без подстановки) этому
    /// свойству: негодное объявление отбрасывается разбором (CSS Syntax
    /// §4.1.7 / css-cascade-5 §4) и в каскад не попадает.
    pub fn accepts(self, value: &str, vcx: ValueCx<'_>) -> bool {
        crate::style::computed::Computed::accepts_longhand(self, value, vcx)
    }
}
