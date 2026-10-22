//! Сокращения реестра: имена и их разбор по имени.

/// Сокращение реестра.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShorthandId {
    /// css-box-4 §3.2.
    Margin,
    /// css-box-4 §4.2.
    Padding,
    /// css-position-3 §3.2 (`inset`).
    Inset,
    /// css-logical-1 §4.2-4.4: пары по оси.
    MarginBlock,
    MarginInline,
    PaddingBlock,
    PaddingInline,
    InsetBlock,
    InsetInline,
    /// css-backgrounds-3 §4.4: `border`, стороны и части по сторонам.
    Border,
    BorderTop,
    BorderRight,
    BorderBottom,
    BorderLeft,
    BorderWidth,
    BorderStyle,
    BorderColor,
    /// css-logical-1 §4.4: логические кромки.
    BorderBlock,
    BorderInline,
    BorderBlockStart,
    BorderBlockEnd,
    BorderInlineStart,
    BorderInlineEnd,
    BorderBlockWidth,
    BorderBlockStyle,
    BorderBlockColor,
    BorderInlineWidth,
    BorderInlineStyle,
    BorderInlineColor,
    /// css-ui-4 §outline.
    Outline,
    /// css-backgrounds-4 §background: слои фона и цвет.
    Background,
    /// css-backgrounds-4 §3.6: оси положения слоёв.
    BackgroundPosition,
    /// css-masking-1 §7.9: слои маски и сброс `mask-border`.
    Mask,
    /// css-masking-1 §8.7.
    MaskBorder,
    /// css-fonts-4 §font-prop: шрифт и сброс подсвойств.
    Font,
    /// css-fonts-4 §6.11.
    FontVariant,
    /// css-fonts-4 §font-synthesis.
    FontSynthesis,
    /// css-text-4 §3: схлопывание и перенос.
    WhiteSpace,
    /// css-text-4 §5.1.
    TextWrap,
    /// css-text-decor-4 §2.6.
    TextDecoration,
    /// css-text-decor-4 §3.3.
    TextEmphasis,
}

const SHORTHANDS: &[(&str, ShorthandId)] = &[
    ("margin", ShorthandId::Margin),
    ("padding", ShorthandId::Padding),
    ("inset", ShorthandId::Inset),
    ("margin-block", ShorthandId::MarginBlock),
    ("margin-inline", ShorthandId::MarginInline),
    ("padding-block", ShorthandId::PaddingBlock),
    ("padding-inline", ShorthandId::PaddingInline),
    ("inset-block", ShorthandId::InsetBlock),
    ("inset-inline", ShorthandId::InsetInline),
    ("border", ShorthandId::Border),
    ("border-top", ShorthandId::BorderTop),
    ("border-right", ShorthandId::BorderRight),
    ("border-bottom", ShorthandId::BorderBottom),
    ("border-left", ShorthandId::BorderLeft),
    ("border-width", ShorthandId::BorderWidth),
    ("border-style", ShorthandId::BorderStyle),
    ("border-color", ShorthandId::BorderColor),
    ("border-block", ShorthandId::BorderBlock),
    ("border-inline", ShorthandId::BorderInline),
    ("border-block-start", ShorthandId::BorderBlockStart),
    ("border-block-end", ShorthandId::BorderBlockEnd),
    ("border-inline-start", ShorthandId::BorderInlineStart),
    ("border-inline-end", ShorthandId::BorderInlineEnd),
    ("border-block-width", ShorthandId::BorderBlockWidth),
    ("border-block-style", ShorthandId::BorderBlockStyle),
    ("border-block-color", ShorthandId::BorderBlockColor),
    ("border-inline-width", ShorthandId::BorderInlineWidth),
    ("border-inline-style", ShorthandId::BorderInlineStyle),
    ("border-inline-color", ShorthandId::BorderInlineColor),
    ("outline", ShorthandId::Outline),
    ("background", ShorthandId::Background),
    ("background-position", ShorthandId::BackgroundPosition),
    ("mask", ShorthandId::Mask),
    ("mask-border", ShorthandId::MaskBorder),
    ("font", ShorthandId::Font),
    ("font-variant", ShorthandId::FontVariant),
    ("font-synthesis", ShorthandId::FontSynthesis),
    ("white-space", ShorthandId::WhiteSpace),
    ("text-wrap", ShorthandId::TextWrap),
    ("text-decoration", ShorthandId::TextDecoration),
    ("text-emphasis", ShorthandId::TextEmphasis),
];

impl ShorthandId {
    pub fn from_name(name: &str) -> Option<Self> {
        SHORTHANDS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
    }

    pub fn name(self) -> &'static str {
        SHORTHANDS
            .iter()
            .find(|(_, s)| *s == self)
            .map_or("", |(n, _)| n)
    }
}
