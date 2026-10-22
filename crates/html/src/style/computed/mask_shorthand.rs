//! Вычисленный стиль: слои `mask-image` (css-masking-1 §7.2). Сокращение
//! `mask` раскрывает разбор (`style::css::property`).

use super::{ImageSource, split_outside_parens};
use crate::document::state::ValueCx;

/// Keep absent layers in the list: their indices select geometry and composition.
pub(crate) fn images(value: &str, vcx: ValueCx<'_>) -> Option<Vec<ImageSource>> {
    crate::style::css::split_args(value)
        .into_iter()
        .map(|layer| {
            split_outside_parens(layer).into_iter().find_map(|word| {
                if word == "none" {
                    Some(ImageSource::Invalid)
                } else {
                    ImageSource::parse(&word, vcx)
                }
            })
        })
        .collect()
}
