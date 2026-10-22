//! Сокращения реестра и их раскрытие при разборе (Blink
//! `Shorthand::ParseShorthand`, Stylo `shorthands/*.mako.rs` `parse_into`):
//! годное сокращение даёт ВСЕ свои длинные свойства, негодное — ничего.
//! Общее слово переходит на каждое длинное свойство, значение с подстановкой
//! — тоже, текстом сокращения целиком (разбор после подстановки).

use super::{CssWide, LonghandId, Specified, has_substitution};
use crate::document::state::ValueCx;
use LonghandId::*;

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

    /// Длинные свойства в порядке компонентов значения; у сокращений вида
    /// `<width> || <style> || <color>` — см. [`Self::line_longhands`].
    pub fn longhands(self) -> &'static [LonghandId] {
        match self {
            Self::Margin => &[MarginTop, MarginRight, MarginBottom, MarginLeft],
            Self::Padding => &[PaddingTop, PaddingRight, PaddingBottom, PaddingLeft],
            Self::Inset => &[Top, Right, Bottom, Left],
            Self::MarginBlock => &[MarginBlockStart, MarginBlockEnd],
            Self::MarginInline => &[MarginInlineStart, MarginInlineEnd],
            Self::PaddingBlock => &[PaddingBlockStart, PaddingBlockEnd],
            Self::PaddingInline => &[PaddingInlineStart, PaddingInlineEnd],
            Self::InsetBlock => &[InsetBlockStart, InsetBlockEnd],
            Self::InsetInline => &[InsetInlineStart, InsetInlineEnd],
            Self::BorderWidth => &[
                BorderTopWidth,
                BorderRightWidth,
                BorderBottomWidth,
                BorderLeftWidth,
            ],
            Self::BorderStyle => &[
                BorderTopStyle,
                BorderRightStyle,
                BorderBottomStyle,
                BorderLeftStyle,
            ],
            Self::BorderColor => &[
                BorderTopColor,
                BorderRightColor,
                BorderBottomColor,
                BorderLeftColor,
            ],
            Self::BorderBlockWidth => &[BorderBlockStartWidth, BorderBlockEndWidth],
            Self::BorderBlockStyle => &[BorderBlockStartStyle, BorderBlockEndStyle],
            Self::BorderBlockColor => &[BorderBlockStartColor, BorderBlockEndColor],
            Self::BorderInlineWidth => &[BorderInlineStartWidth, BorderInlineEndWidth],
            Self::BorderInlineStyle => &[BorderInlineStartStyle, BorderInlineEndStyle],
            Self::BorderInlineColor => &[BorderInlineStartColor, BorderInlineEndColor],
            _ => self.line_longhands(),
        }
    }

    /// Раскрыть значение сокращения в длинные свойства: `(свойство, вид
    /// значения, текст)`. `None` — сокращение негодно и не даёт ничего.
    /// С документом (`vcx`) каждый компонент проверяется грамматикой своего
    /// свойства; без него проверяется лишь форма значения.
    pub fn expand(
        self,
        value: &str,
        vcx: Option<ValueCx<'_>>,
    ) -> Option<Vec<(LonghandId, Specified, String)>> {
        let longhands = self.longhands();
        let whole = |spec: Specified| {
            longhands
                .iter()
                .map(|&id| (id, spec, value.trim().to_string()))
                .collect()
        };
        if let Some(w) = CssWide::parse(value) {
            return Some(whole(Specified::Wide(w)));
        }
        if has_substitution(value) {
            return Some(whole(Specified::Pending(Some(self))));
        }
        let parts = match self.line_parts() {
            Some(_) => self.line_components(value, vcx)?,
            None => self.components(value)?,
        };
        let mut out = Vec::with_capacity(longhands.len());
        for (&id, text) in longhands.iter().zip(parts) {
            if CssWide::parse(&text).is_some() || vcx.is_some_and(|vcx| !id.accepts(&text, vcx)) {
                return None;
            }
            out.push((id, Specified::Value, text));
        }
        Some(out)
    }

    /// Компоненты значения по длинным свойствам. Четыре стороны: 1 значение —
    /// все, 2 — верх/низ и бока, 3 — верх, бока, низ, 4 — по часовой
    /// (css-backgrounds-3 §1.4.1 «4-value syntax»). Пара по оси: 1 значение —
    /// обе стороны, 2 — начало и конец (css-logical-1 §4.2).
    fn components(self, value: &str) -> Option<Vec<String>> {
        let v = crate::style::computed::split_outside_parens(value.trim());
        let pick: &[usize] = match (self.longhands().len(), v.len()) {
            (4, 1) => &[0, 0, 0, 0],
            (4, 2) => &[0, 1, 0, 1],
            (4, 3) => &[0, 1, 2, 1],
            (4, 4) => &[0, 1, 2, 3],
            (2, 1) => &[0, 0],
            (2, 2) => &[0, 1],
            _ => return None,
        };
        Some(pick.iter().map(|&i| v[i].clone()).collect())
    }

    /// Текст компонента длинного свойства `id` из значения сокращения — после
    /// подстановки (`Specified::Pending`). `None` — значение негодно.
    pub fn component_of(self, id: LonghandId, value: &str, vcx: ValueCx<'_>) -> Option<String> {
        self.expand(value, Some(vcx))?
            .into_iter()
            .find(|(long, spec, _)| *long == id && *spec == Specified::Value)
            .map(|(_, _, text)| text)
    }
}
