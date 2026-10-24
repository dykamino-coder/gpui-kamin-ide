//! Сокращения разбора фиксированных тестовых документов; production возвращает Result.

use super::*;

// Только тестовые сокращения: production всегда получает Result.
#[cfg(test)]
pub(crate) fn parse(html: &str, css: &str) -> Parsed {
    try_parse(html, css).unwrap()
}
#[cfg(test)]
pub(crate) fn parse_xml(xml: &str, css: &str) -> Parsed {
    try_parse_xml(xml, css).unwrap()
}
#[cfg(test)]
pub(crate) fn parse_current(
    html: &str,
    css: &str,
    media: crate::style::css::Media,
    doc: &std::rc::Rc<crate::document::state::DocState>,
) -> Vec<Node> {
    convert(
        &input::Admitted::parse(html, false).unwrap(),
        css,
        media,
        doc,
    )
}
