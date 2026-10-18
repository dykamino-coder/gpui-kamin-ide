//! Recognize dimensionless color images independently of URL fallback.

use crate::document::state::ValueCx;
pub(crate) fn parse<'a>(raw: &'a str, vcx: ValueCx<'_>) -> Option<&'a str> {
    super::image_function::color_only(raw, vcx)
}
