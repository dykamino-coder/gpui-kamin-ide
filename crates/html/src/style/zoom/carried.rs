//! Наследуемые длины в точках, которые родитель передаёт детям уже в своём зуме.

use crate::style::computed::{Computed, Shadow};
use crate::style::values::value::px_strict;

/// Наследуемые длины родителя в точках (уже домноженные на его зум).
/// Длина не в точках (число `line-height`, доля) не переносится: ребёнок
/// наследует не точки, и домножать нечего.
#[derive(Clone, Default)]
pub(super) struct Carried {
    pub(super) line_height: Option<f32>,
    pub(super) letter_spacing: Option<f32>,
    pub(super) word_spacing: Option<f32>,
    pub(super) text_indent: Option<f32>,
    pub(super) tab_size: Option<f32>,
    pub(super) text_shadow: Option<Shadow>,
    /// Остальные тени списка `text-shadow`: список наследуется целиком.
    pub(super) text_shadow_rest: Vec<Shadow>,
    pub(super) border_spacing: Option<(Option<f32>, Option<f32>)>,
    pub(super) underline_offset: Option<f32>,
}

impl Carried {
    /// Длины, которые вычисленный стиль родителя передаёт детям.
    pub(super) fn of(parent: Option<&Computed>) -> Self {
        let Some(c) = parent else {
            return Self::default();
        };
        let px = px_strict;
        Self {
            line_height: px(c.line_height),
            letter_spacing: px(c.letter_spacing),
            word_spacing: px(c.word_spacing),
            text_indent: px(c.text_indent),
            tab_size: px(c.tab_size_len),
            text_shadow: c.text_shadow.clone(),
            text_shadow_rest: c.text_shadow_rest.clone(),
            border_spacing: c.border_spacing.map(|(row, col)| (px(row), px(col))),
            underline_offset: match c.underline_offset {
                Some(crate::style::computed::DecorLen::Px(v)) => Some(v),
                _ => None,
            },
        }
    }
}
