//! Parse the input image and filter list of CSS filter() image values.

/// Splits `filter(url(x), list)` into the decoded url and the filter list.
pub(crate) fn parse_filter_image(src: &str) -> Option<(String, String)> {
    let rest = src.trim().strip_prefix("filter(")?;
    let inner = &rest[..rest.rfind(')')?];
    let parts = crate::style::css::split_args(inner);
    let image = parts.first()?.trim();
    if parts.len() < 2 {
        return None;
    }
    let input = if image.to_ascii_lowercase().starts_with("url(") {
        crate::style::computed::parse_url(image)?
    } else if crate::style::computed::parse_gradient(image).is_some() {
        image.to_string()
    } else {
        return None;
    };
    Some((input, parts[1..].join(" ")))
}
