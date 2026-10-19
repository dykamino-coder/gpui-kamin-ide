//! Градиенты и image-set: разбор, угол, растр, image-resolution.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod current_color;
mod direction;
mod image_set;
mod parse;
pub(crate) mod stops;
pub(crate) use current_color::{
    parse_gradient, parse_gradient_with_current, resolve_gradient_colors,
};
pub(super) use direction::corner_direction;
use direction::gradient_direction;
pub(super) use image_set::image_set_pick;
use parse::gradient_angle;

/// Conic/repeating gradients and cross-fades require an image tile.
pub(crate) fn gradient_as_raster(v: &str) -> bool {
    // `cross-fade()` (css-images-4 §2.6) — смесь картинок растром той же
    // плиткой (`background::rasterize_cross_fade`).
    v.starts_with("cross-fade(")
        || v.starts_with("conic-gradient(")
        || v.starts_with("repeating-linear-gradient(")
        || v.starts_with("repeating-radial-gradient(")
        || v.starts_with("repeating-conic-gradient(")
}

/// Пересчитать фильтром цвета стопов в ЗАПИСИ растрового градиента: такие
/// градиенты живут строкой в `bg_image`, а не полем `gradient`. Слова, не
/// являющиеся цветом (направление, позиции, `from`/`at`), остаются как есть.
pub(crate) fn filter_gradient_text(raw: &str, f: &Filter, vcx: ValueCx<'_>) -> String {
    let (Some(open), Some(close)) = (raw.find('('), raw.rfind(')')) else {
        return raw.to_string();
    };
    if close <= open {
        return raw.to_string();
    }
    let parts: Vec<String> = crate::style::css::split_args(&raw[open + 1..close])
        .into_iter()
        .map(|part| {
            split_outside_parens(part)
                .into_iter()
                .map(|w| match Color::parse(&w, vcx) {
                    Some(c) => {
                        let c = f.apply(c);
                        format!(
                            "rgba({},{},{},{})",
                            (c.r * 255.0).round(),
                            (c.g * 255.0).round(),
                            (c.b * 255.0).round(),
                            c.a
                        )
                    }
                    None => w,
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    format!("{}{})", &raw[..=open], parts.join(", "))
}
