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
    /// css-flexbox-1 §7.1.
    Flex,
    /// css-flexbox-1 §5.3.
    FlexFlow,
    /// css-gaps-1 §2.2 (устаревшее имя `grid-gap`).
    Gap,
    /// css-align-3 §5.3.
    PlaceContent,
    /// css-align-3 §6.3.
    PlaceItems,
    /// css-align-3 §6.4.
    PlaceSelf,
    /// css-grid-2 §8.4: грани размещения.
    GridRow,
    GridColumn,
    GridArea,
    /// css-grid-2 §7.4.
    GridTemplate,
    /// css-grid-2 §7.8.
    Grid,
    /// css-multicol-2 §columns.
    Columns,
    /// css-gaps-1 §rule-shorthands и обе оси `rule-*`.
    ColumnRule,
    RowRule,
    Rule,
    RuleWidth,
    RuleStyle,
    RuleColor,
    RuleBreak,
    RuleVisibilityItems,
    /// css-gaps-1 §inset: втяжки концов и стыков.
    ColumnRuleInset,
    RowRuleInset,
    RuleInset,
    ColumnRuleInsetCap,
    RowRuleInsetCap,
    RuleInsetCap,
    ColumnRuleInsetJunction,
    RowRuleInsetJunction,
    RuleInsetJunction,
    ColumnRuleInsetStart,
    RowRuleInsetStart,
    RuleInsetStart,
    ColumnRuleInsetEnd,
    RowRuleInsetEnd,
    RuleInsetEnd,
    /// css-lists-3 §3.6.
    ListStyle,
    /// css-text-4 §7.1: `text-align-all` и `text-align-last`.
    TextAlign,
    /// css-inline-3 §4.1.
    TextBox,
    /// css-overflow-4 §5.2-5.3: `max-lines`, `block-ellipsis`, `continue`.
    LineClamp,
    WebkitLineClamp,
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
    ("flex", ShorthandId::Flex),
    ("flex-flow", ShorthandId::FlexFlow),
    ("gap", ShorthandId::Gap),
    ("place-content", ShorthandId::PlaceContent),
    ("place-items", ShorthandId::PlaceItems),
    ("place-self", ShorthandId::PlaceSelf),
    ("grid-row", ShorthandId::GridRow),
    ("grid-column", ShorthandId::GridColumn),
    ("grid-area", ShorthandId::GridArea),
    ("grid-template", ShorthandId::GridTemplate),
    ("grid", ShorthandId::Grid),
    ("columns", ShorthandId::Columns),
    ("column-rule", ShorthandId::ColumnRule),
    ("row-rule", ShorthandId::RowRule),
    ("rule", ShorthandId::Rule),
    ("rule-width", ShorthandId::RuleWidth),
    ("rule-style", ShorthandId::RuleStyle),
    ("rule-color", ShorthandId::RuleColor),
    ("rule-break", ShorthandId::RuleBreak),
    ("rule-visibility-items", ShorthandId::RuleVisibilityItems),
    ("column-rule-inset", ShorthandId::ColumnRuleInset),
    ("row-rule-inset", ShorthandId::RowRuleInset),
    ("rule-inset", ShorthandId::RuleInset),
    ("column-rule-inset-cap", ShorthandId::ColumnRuleInsetCap),
    ("row-rule-inset-cap", ShorthandId::RowRuleInsetCap),
    ("rule-inset-cap", ShorthandId::RuleInsetCap),
    (
        "column-rule-inset-junction",
        ShorthandId::ColumnRuleInsetJunction,
    ),
    ("row-rule-inset-junction", ShorthandId::RowRuleInsetJunction),
    ("rule-inset-junction", ShorthandId::RuleInsetJunction),
    ("column-rule-inset-start", ShorthandId::ColumnRuleInsetStart),
    ("row-rule-inset-start", ShorthandId::RowRuleInsetStart),
    ("rule-inset-start", ShorthandId::RuleInsetStart),
    ("column-rule-inset-end", ShorthandId::ColumnRuleInsetEnd),
    ("row-rule-inset-end", ShorthandId::RowRuleInsetEnd),
    ("rule-inset-end", ShorthandId::RuleInsetEnd),
    ("list-style", ShorthandId::ListStyle),
    ("text-align", ShorthandId::TextAlign),
    ("text-box", ShorthandId::TextBox),
    ("line-clamp", ShorthandId::LineClamp),
    ("-webkit-line-clamp", ShorthandId::WebkitLineClamp),
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
