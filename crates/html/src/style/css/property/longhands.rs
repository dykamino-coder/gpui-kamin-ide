//! Таблица длинных свойств реестра: имя, начальное значение (css-box-4,
//! css-position-3, css-sizing-3, css-logical-1, css-backgrounds-3/4,
//! css-masking-1, css-ui-4, css-fonts-4, css-inline-3, css-text-4,
//! css-text-decor-4, css-flexbox-1, css-align-3, css-grid-2, css-multicol-2,
//! css-gaps-1), наследование, грамматика и место в логической группе (css-logical-1 §4:
//! логическое и физическое свойство одной стороны делят вычисленное
//! значение). Прочие наследуемые свойства — продолжение таблицы в
//! `inherited_rows`.

use super::Grammar;

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

#[macro_use]
mod inherited_rows;

with_inherited_rows! {
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
    FontStyle = "font-style", "normal", true, Font, N;
    FontVariantLigatures = "font-variant-ligatures", "normal", true, Font, N;
    FontVariantCaps = "font-variant-caps", "normal", true, Font, N;
    FontVariantAlternates = "font-variant-alternates", "normal", true, Font, N;
    FontVariantNumeric = "font-variant-numeric", "normal", true, Font, N;
    FontVariantEastAsian = "font-variant-east-asian", "normal", true, Font, N;
    FontVariantPosition = "font-variant-position", "normal", true, Font, N;
    FontVariantEmoji = "font-variant-emoji", "normal", true, Font, N;
    FontWeight = "font-weight", "normal", true, Font, N;
    FontWidth = "font-width", "normal", true, Font, N;
    FontSize = "font-size", "medium", true, Font, N;
    LineHeight = "line-height", "normal", true, Font, N;
    FontFamily = "font-family", "serif", true, Font, N;
    FontKerning = "font-kerning", "auto", true, Font, N;
    FontSizeAdjust = "font-size-adjust", "none", true, Font, N;
    FontFeatureSettings = "font-feature-settings", "normal", true, Font, N;
    FontSynthesisWeight = "font-synthesis-weight", "auto", true, Font, N;
    FontSynthesisStyle = "font-synthesis-style", "auto", true, Font, N;
    FontSynthesisSmallCaps = "font-synthesis-small-caps", "auto", true, Font, N;
    FontSynthesisPosition = "font-synthesis-position", "auto", true, Font, N;
    WhiteSpaceCollapse = "white-space-collapse", "collapse", true, Text, N;
    TextWrapMode = "text-wrap-mode", "wrap", true, Text, N;
    TextWrapStyle = "text-wrap-style", "auto", true, Text, N;
    TextEmphasisStyle = "text-emphasis-style", "none", true, Text, N;
    TextEmphasisColor = "text-emphasis-color", "currentcolor", true, Text, N;
    TextDecorationLine = "text-decoration-line", "none", false, TextDecor, N;
    TextDecorationStyle = "text-decoration-style", "solid", false, TextDecor, N;
    TextDecorationColor = "text-decoration-color", "currentcolor", false, TextDecor, N;
    TextDecorationThickness = "text-decoration-thickness", "auto", false, TextDecor, N;
    FlexGrow = "flex-grow", "0", false, Flex, N;
    FlexShrink = "flex-shrink", "1", false, Flex, N;
    FlexBasis = "flex-basis", "auto", false, Flex, N;
    FlexDirection = "flex-direction", "row", false, Flex, N;
    FlexWrap = "flex-wrap", "nowrap", false, Flex, N;
    AlignContent = "align-content", "normal", false, Align, N;
    JustifyContent = "justify-content", "normal", false, Align, N;
    AlignItems = "align-items", "normal", false, Align, N;
    JustifyItems = "justify-items", "legacy", false, Align, N;
    AlignSelf = "align-self", "auto", false, Align, N;
    JustifySelf = "justify-self", "auto", false, Align, N;
    RowGap = "row-gap", "normal", false, Gap, N;
    ColumnGap = "column-gap", "normal", false, Gap, N;
    GridTemplateRows = "grid-template-rows", "none", false, Grid, N;
    GridTemplateColumns = "grid-template-columns", "none", false, Grid, N;
    GridTemplateAreas = "grid-template-areas", "none", false, Grid, N;
    GridAutoRows = "grid-auto-rows", "auto", false, Grid, N;
    GridAutoColumns = "grid-auto-columns", "auto", false, Grid, N;
    GridAutoFlow = "grid-auto-flow", "row", false, Grid, N;
    GridRowStart = "grid-row-start", "auto", false, Grid, N;
    GridColumnStart = "grid-column-start", "auto", false, Grid, N;
    GridRowEnd = "grid-row-end", "auto", false, Grid, N;
    GridColumnEnd = "grid-column-end", "auto", false, Grid, N;
    ColumnWidth = "column-width", "auto", false, Multicol, N;
    ColumnCount = "column-count", "auto", false, Multicol, N;
    ColumnHeight = "column-height", "auto", false, Multicol, N;
    ColumnWrap = "column-wrap", "auto", false, Multicol, N;
    ColumnRuleWidth = "column-rule-width", "medium", false, GapRule, N;
    ColumnRuleStyle = "column-rule-style", "none", false, GapRule, N;
    ColumnRuleColor = "column-rule-color", "currentcolor", false, GapRule, N;
    RowRuleWidth = "row-rule-width", "medium", false, GapRule, N;
    RowRuleStyle = "row-rule-style", "none", false, GapRule, N;
    RowRuleColor = "row-rule-color", "currentcolor", false, GapRule, N;
    ColumnRuleBreak = "column-rule-break", "normal", false, GapRule, N;
    RowRuleBreak = "row-rule-break", "normal", false, GapRule, N;
    ColumnRuleVisibilityItems = "column-rule-visibility-items", "normal", false, GapRule, N;
    RowRuleVisibilityItems = "row-rule-visibility-items", "normal", false, GapRule, N;
    ColumnRuleInsetCapStart = "column-rule-inset-cap-start", "0", false, GapRule, N;
    ColumnRuleInsetCapEnd = "column-rule-inset-cap-end", "0", false, GapRule, N;
    ColumnRuleInsetJunctionStart = "column-rule-inset-junction-start", "0", false, GapRule, N;
    ColumnRuleInsetJunctionEnd = "column-rule-inset-junction-end", "0", false, GapRule, N;
    RowRuleInsetCapStart = "row-rule-inset-cap-start", "0", false, GapRule, N;
    RowRuleInsetCapEnd = "row-rule-inset-cap-end", "0", false, GapRule, N;
    RowRuleInsetJunctionStart = "row-rule-inset-junction-start", "0", false, GapRule, N;
    RowRuleInsetJunctionEnd = "row-rule-inset-junction-end", "0", false, GapRule, N;
}

mod info;
mod set;
pub use set::LonghandSet;
