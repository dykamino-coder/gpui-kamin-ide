//! Сокращения реестра и их раскрытие при разборе (Blink
//! `Shorthand::ParseShorthand`, Stylo `shorthands/*.mako.rs` `parse_into`):
//! годное сокращение даёт ВСЕ свои длинные свойства, негодное — ничего.
//! Общее слово переходит на каждое длинное свойство, значение с подстановкой
//! — тоже, текстом сокращения целиком (разбор после подстановки).

pub use super::shorthand_ids::ShorthandId;
use super::{CssWide, LonghandId, Specified, has_substitution};
use crate::document::state::ValueCx;
use LonghandId::*;

impl ShorthandId {
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
            Self::Background => &[
                BackgroundImage,
                BackgroundPositionX,
                BackgroundPositionY,
                BackgroundSize,
                BackgroundRepeat,
                BackgroundAttachment,
                BackgroundOrigin,
                BackgroundClip,
                BackgroundColor,
            ],
            Self::BackgroundPosition => &[BackgroundPositionX, BackgroundPositionY],
            Self::Mask => super::mask_shorthands::MASK,
            Self::MaskBorder => &[
                MaskBorderSource,
                MaskBorderSlice,
                MaskBorderWidth,
                MaskBorderOutset,
                MaskBorderRepeat,
                MaskBorderMode,
            ],
            _ => self
                .font_longhands()
                .or_else(|| self.text_longhands())
                .or_else(|| self.layout_longhands())
                .or_else(|| self.inline_longhands())
                .unwrap_or_else(|| self.line_longhands()),
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
        let parts = match self {
            Self::Background => super::layer_shorthands::background(value, vcx)?,
            Self::BackgroundPosition => super::layer_shorthands::background_position(value, vcx)?,
            Self::Mask => super::mask_shorthands::mask(value, vcx)?,
            Self::MaskBorder => super::mask_shorthands::mask_border(value, vcx)?,
            Self::Font => super::font_shorthands::font(value, vcx)?,
            Self::FontVariant => super::variant_shorthands::font_variant(value)?,
            Self::FontSynthesis => super::variant_shorthands::font_synthesis(value)?,
            Self::WhiteSpace => super::text_shorthands::white_space(value)?,
            Self::TextWrap => super::text_shorthands::text_wrap(value)?,
            Self::TextDecoration => super::text_shorthands::text_decoration(value)?,
            Self::TextEmphasis => super::text_shorthands::text_emphasis(value)?,
            _ if self.layout_longhands().is_some() => self.layout_parts(value, vcx)?,
            _ if self.inline_longhands().is_some() => self.inline_parts(value)?,
            _ if self.line_parts().is_some() => self.line_components(value, vcx)?,
            _ => self.components(value)?,
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
