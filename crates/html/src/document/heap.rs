//! Память документа по владельцам (`heap-report`, docs/memprof.md): обход по
//! запросу стенда; в обычной сборке модуля нет.

use super::Document;
use kamin_heap::{HeapSize, Ops};

impl Document {
    /// Блоки кучи документа по путям отчёта: DOM, исходное дерево разбора,
    /// стиль, сохраняемые записи.
    pub fn heap_report(&self, ops: &mut Ops) {
        ops.at("dom/nodes", |ops| self.nodes.heap_size(ops));
        if let Some(source) = self.state.values.source_dom() {
            ops.at("dom/source-tree", |ops| {
                kamin_style::dom::heap::source_tree(&source, ops)
            });
        }
        ops.at("style/computed", |ops| self.root.heap_size(ops));
        ops.at("text/paragraph-measures", |ops| {
            self.state.paragraph.paragraphs.heap_size(ops)
        });
        ops.at("document/other", |ops| self.theme_css.heap_size(ops));
    }
}
