//! Сокращения вида `<line-width> || <line-style> || <color>`: `border`, его
//! стороны, логические кромки (css-backgrounds-3 §4.4, css-logical-1 §4.4) и
//! `outline` (css-ui-4 §outline). Части идут в любом порядке и не больше раза
//! каждая; опущенная часть получает начальное значение. `border` сверх своей
//! грамматики сбрасывает `border-image` (css-backgrounds-3 §4.4: «also resets
//! border-image to its initial value»; Blink `border` longhands, Stylo
//! `border` `reset_border_image`).

use super::{Grammar, LonghandId, ShorthandId};
use crate::document::state::ValueCx;
use LonghandId::*;

const BORDER: &[LonghandId] = &[
    BorderTopWidth,
    BorderRightWidth,
    BorderBottomWidth,
    BorderLeftWidth,
    BorderTopStyle,
    BorderRightStyle,
    BorderBottomStyle,
    BorderLeftStyle,
    BorderTopColor,
    BorderRightColor,
    BorderBottomColor,
    BorderLeftColor,
    BorderImageSource,
    BorderImageSlice,
    BorderImageWidth,
    BorderImageOutset,
    BorderImageRepeat,
];

const BORDER_BLOCK: &[LonghandId] = &[
    BorderBlockStartWidth,
    BorderBlockEndWidth,
    BorderBlockStartStyle,
    BorderBlockEndStyle,
    BorderBlockStartColor,
    BorderBlockEndColor,
];

const BORDER_INLINE: &[LonghandId] = &[
    BorderInlineStartWidth,
    BorderInlineEndWidth,
    BorderInlineStartStyle,
    BorderInlineEndStyle,
    BorderInlineStartColor,
    BorderInlineEndColor,
];

impl ShorthandId {
    /// Число сторон сокращения вида `<width> || <style> || <color>`; `None`
    /// — сокращение другого вида.
    pub(super) fn line_parts(self) -> Option<usize> {
        Some(match self {
            Self::Border => 4,
            Self::BorderBlock | Self::BorderInline => 2,
            Self::BorderTop
            | Self::BorderRight
            | Self::BorderBottom
            | Self::BorderLeft
            | Self::BorderBlockStart
            | Self::BorderBlockEnd
            | Self::BorderInlineStart
            | Self::BorderInlineEnd
            | Self::Outline => 1,
            _ => return None,
        })
    }

    /// Длинные свойства: толщины сторон, их стили, их цвета, затем
    /// сбрасываемые свойства вне грамматики.
    pub(super) fn line_longhands(self) -> &'static [LonghandId] {
        match self {
            Self::Border => BORDER,
            Self::BorderBlock => BORDER_BLOCK,
            Self::BorderInline => BORDER_INLINE,
            Self::BorderTop => &[BorderTopWidth, BorderTopStyle, BorderTopColor],
            Self::BorderRight => &[BorderRightWidth, BorderRightStyle, BorderRightColor],
            Self::BorderBottom => &[BorderBottomWidth, BorderBottomStyle, BorderBottomColor],
            Self::BorderLeft => &[BorderLeftWidth, BorderLeftStyle, BorderLeftColor],
            Self::BorderBlockStart => &[
                BorderBlockStartWidth,
                BorderBlockStartStyle,
                BorderBlockStartColor,
            ],
            Self::BorderBlockEnd => &[
                BorderBlockEndWidth,
                BorderBlockEndStyle,
                BorderBlockEndColor,
            ],
            Self::BorderInlineStart => &[
                BorderInlineStartWidth,
                BorderInlineStartStyle,
                BorderInlineStartColor,
            ],
            Self::BorderInlineEnd => &[
                BorderInlineEndWidth,
                BorderInlineEndStyle,
                BorderInlineEndColor,
            ],
            Self::Outline => &[OutlineWidth, OutlineStyle, OutlineColor],
            _ => &[],
        }
    }

    /// Тексты длинных свойств по порядку [`Self::line_longhands`]. Часть
    /// узнаётся грамматикой своего свойства: с документом — его разбором,
    /// без документа (CSS самого движка) — по форме: стиль — ключевое слово,
    /// толщина — слово толщины или число/математическая функция, остальное —
    /// цвет. `None` — значение негодно.
    pub(super) fn line_components(
        self,
        value: &str,
        vcx: Option<ValueCx<'_>>,
    ) -> Option<Vec<String>> {
        let sides = self.line_parts()?;
        let ids = self.line_longhands();
        let (width_id, style_id, color_id) = (ids[0], ids[sides], ids[2 * sides]);
        let tokens = crate::style::computed::split_outside_parens(value.trim());
        if tokens.is_empty() || tokens.len() > 3 {
            return None;
        }
        let ok = |id: LonghandId, t: &str| match vcx {
            Some(vcx) => id.accepts(t, vcx),
            None => (id.grammar() == Grammar::LineWidth) == looks_like_width(t),
        };
        let mut parts: [Option<String>; 3] = [None, None, None];
        for t in &tokens {
            let t = t.trim();
            let slot = if parts[1].is_none() && crate::style::computed::line_style_ok(style_id, t) {
                1
            } else if parts[0].is_none() && ok(width_id, t) {
                0
            } else if parts[2].is_none() && ok(color_id, t) {
                2
            } else {
                return None;
            };
            parts[slot] = Some(t.to_string());
        }
        let mut out = Vec::with_capacity(ids.len());
        for (part, id) in parts.iter().zip([width_id, style_id, color_id]) {
            let text = part.clone().unwrap_or_else(|| id.initial().to_string());
            out.extend(std::iter::repeat_n(text, sides));
        }
        out.extend(ids[3 * sides..].iter().map(|id| id.initial().to_string()));
        Some(out)
    }
}

/// Форма толщины без разбора: слово толщины, число с единицей или без,
/// математическая функция (css-values-4 §10).
fn looks_like_width(t: &str) -> bool {
    let lower = t.to_ascii_lowercase();
    let mut chars = lower.chars();
    matches!(lower.as_str(), "thin" | "medium" | "thick")
        || ["calc(", "min(", "max(", "clamp("]
            .iter()
            .any(|f| lower.starts_with(f))
        || match chars.next() {
            Some(c) if c.is_ascii_digit() || c == '.' => true,
            Some('+' | '-') => chars.next().is_some_and(|c| c.is_ascii_digit() || c == '.'),
            _ => false,
        }
}
