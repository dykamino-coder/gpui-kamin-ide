//! Градиенты и image-set: разбор, угол, растр, image-resolution.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod current_color;
mod direction;
mod image_set;
mod interpolation;
pub(super) use interpolation::{interpolation_method, legacy_srgb_color};
mod lengths;
mod parse;
pub(crate) mod stops;
pub(crate) use current_color::parse_gradient;
pub(super) use direction::corner_direction;
use direction::gradient_direction;
pub(super) use image_set::image_set_pick;
use parse::gradient_angle;
