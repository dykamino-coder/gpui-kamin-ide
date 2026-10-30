//! Сокращения разбора фиксированных тестовых документов в нижние данные без
//! бэкенда шрифтов; production возвращает Result.

use super::*;

// Только тестовые сокращения: production всегда получает Result.
#[cfg(test)]
pub fn parse(html: &str, css: &str) -> Parsed {
    parse_detached(html, css, false)
}
#[cfg(test)]
pub fn parse_xml(xml: &str, css: &str) -> Parsed {
    parse_detached(xml, css, true)
}
#[cfg(test)]
pub fn parse_current(
    html: &str,
    css: &str,
    media: crate::style::css::Media,
    doc: &std::rc::Rc<crate::values::DocumentValues>,
) -> Vec<Node> {
    convert(
        &input::Admitted::parse(html, false).unwrap(),
        css,
        media,
        doc,
    )
    .unwrap()
}

#[cfg(test)]
fn parse_detached(markup: &str, css: &str, xml: bool) -> Parsed {
    let media = crate::style::css::Media::default();
    let doc = crate::values::DocumentValues::detached(media);
    let nodes = parse_into(markup, css, media, xml, &doc).unwrap();
    Parsed { nodes, doc }
}
