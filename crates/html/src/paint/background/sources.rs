//! Совместимый paint-фасад: все CSS-операнды разрешает ресурсный сервис документа.
use super::Source;
use crate::document::state::ValueCx;
use crate::resources::css_images;
pub fn source(src: &str, vcx: ValueCx<'_>) -> Option<Source> {
    if let Some(raw) = src.strip_prefix("shape:") {
        return Some(Source::Shape {
            raw: raw.to_string(),
        });
    }
    css_images::source(src, vcx).map(Source::from_cpu)
}
pub(crate) fn computed_source(
    image: &crate::style::computed::ImageSource,
    orient: bool,
    vcx: ValueCx<'_>,
) -> Option<Source> {
    css_images::computed_source(image, orient, vcx).map(Source::from_cpu)
}
pub(crate) fn element_source(
    src: &str,
    style: &crate::style::computed::Computed,
    vcx: ValueCx<'_>,
) -> Option<Source> {
    css_images::element_source(src, style, vcx).map(Source::from_cpu)
}
