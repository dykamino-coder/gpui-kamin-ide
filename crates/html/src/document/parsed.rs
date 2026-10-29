//! Разбор без `Document`: узлы и состояние документа целиком (`DocState`),
//! в нижние данные которого разбирает `dom::parse_into`. Ручки `calc()`/`anchor()`
//! в стилях узлов читаются в аренах этого документа (`values`).

use crate::document::state::DocState;
use crate::dom::Node;
use crate::dom::outcome::DocumentFailure;
use crate::style::css::Media;
use crate::values::ValueCx;
use std::rc::Rc;

/// Узлы разбора без `Document` и их документ.
pub struct Parsed {
    pub nodes: Vec<Node>,
    pub doc: Rc<DocState>,
}

impl Parsed {
    /// Контекст значений документа разбора.
    pub fn values(&self) -> ValueCx<'_> {
        self.doc.values()
    }
}

impl std::ops::Deref for Parsed {
    type Target = Vec<Node>;
    fn deref(&self) -> &Vec<Node> {
        &self.nodes
    }
}

impl std::ops::DerefMut for Parsed {
    fn deref_mut(&mut self) -> &mut Vec<Node> {
        &mut self.nodes
    }
}

/// Разобрать фрагмент и вернуть корневые узлы.
///
/// `extra_css` — таблица уровня приложения (тема чата), применяется до
/// `<style>` документа и до `style=""`.
pub fn try_parse(html: &str, extra_css: &str) -> Result<Parsed, DocumentFailure> {
    try_parse_media(html, extra_css, Media::default())
}

/// Разбор без Document, с полным DOM и тем же отчётом DocState.
pub fn try_parse_media(
    html: &str,
    extra_css: &str,
    media: Media,
) -> Result<Parsed, DocumentFailure> {
    parse_mode(html, extra_css, media, false)
}

/// XML-вход без HTML-восстановления.
pub fn try_parse_xml(markup: &str, css: &str) -> Result<Parsed, DocumentFailure> {
    parse_mode(markup, css, Media::default(), true)
}

fn parse_mode(
    html: &str,
    extra_css: &str,
    media: Media,
    xml: bool,
) -> Result<Parsed, DocumentFailure> {
    let doc = DocState::new(media);
    let nodes = crate::dom::parse_into(html, extra_css, media, xml, &doc.values)?;
    Ok(Parsed { nodes, doc })
}

// Только тестовые сокращения: production всегда получает Result.
#[cfg(test)]
pub(crate) fn parse(html: &str, css: &str) -> Parsed {
    try_parse(html, css).unwrap()
}
