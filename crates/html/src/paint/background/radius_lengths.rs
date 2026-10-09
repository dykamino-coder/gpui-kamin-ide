//! Carry calculated length-percentage radii to the border-box rasterizer.

use crate::style::values::value::{Len, calc_get};

pub(super) fn token(length: Option<Len>) -> String {
    match length {
        Some(Len::Px(v)) => format!("{v}"),
        Some(Len::Pct(p)) => format!("{}%", p * 100.0),
        Some(Len::Calc(i)) => match calc_get(i).pct_px() {
            // This is an internal rrect token, not serialized CSS. Its two
            // terms stay separate until the axis's percentage basis is known.
            Some((percentage, pixels)) => format!("{pixels}:{percentage}"),
            None => "0".to_string(),
        },
        _ => "0".to_string(),
    }
}

pub(super) fn resolve(token: &str, basis: f32, scale: f32) -> Option<f32> {
    let value = if let Some((pixels, percentage)) = token.split_once(':') {
        pixels.parse::<f32>().ok()? * scale + percentage.parse::<f32>().ok()? * basis
    } else if let Some(percentage) = token.strip_suffix('%') {
        percentage.parse::<f32>().ok()? / 100.0 * basis
    } else {
        token.parse::<f32>().ok()? * scale
    };
    // CSS Values 4 §10.12: clamp the complete math result to the radius range.
    Some(value.max(0.0))
}
