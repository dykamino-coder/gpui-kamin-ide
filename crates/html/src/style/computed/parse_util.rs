//! Разборщики значений: выравнивание, шрифт, url, пробелы, тени, border-shape, слои фона, calc-size.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod align;
mod background;
mod border;
mod font;
mod shadow;
mod sizing;
mod text;
mod tokens;
pub(super) use align::{align_keyword, is_safe, parse_align, parse_justify};
pub(crate) use background::{BG_LIST_KEYS, background_layers, top_level_comma};
pub(super) use background::{background_shorthand_valid, pct_px_pair, split_image_func};
pub(super) use border::{
    corner_shape_param, corner_shape_shorthand, four, outline_width_of, parse_border_shape,
    parse_overflow, uniform_round,
};
pub use font::{GENERIC_SANS, generic_family};
pub(crate) use font::{family_name_ok, feature_list, font_lengths_to_px, is_generic};
pub(super) use font::{font_size_token, font_slash, has_font_units, split_font, stretch_keyword};
pub(super) use shadow::{box_shadow_valid, parse_shadows};
pub(super) use sizing::{
    CalcSize, assign_size, calc_size_arg, fit_content_arg, flex_factor, parse_view_box,
};
pub(super) use text::{
    collapse_forced_breaks, collapse_segment_breaks, is_quote, split_ws_top, unescape_content,
};
pub(super) use tokens::join_slash;
pub(crate) use tokens::{parse_url, split_outside_parens};
