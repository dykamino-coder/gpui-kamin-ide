//! Recognize dimensionless color images independently of URL fallback.

pub(crate) fn parse(raw: &str) -> Option<&str> {
    super::image_function::color_only(raw)
}
