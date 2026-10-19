//! Узлы разбора без `Document` вместе с их документом.

use super::Node;
use crate::document::state::ValueCx;
use std::rc::Rc;

/// Узлы разбора без `Document` и их документ: ручки `calc()`/`anchor()` в
/// стилях узлов читаются в его аренах (`values`).
pub struct Parsed {
    pub nodes: Vec<Node>,
    pub doc: Rc<crate::document::state::DocState>,
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
