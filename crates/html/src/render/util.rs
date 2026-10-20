//! Public forwarding of subtree text for form controls.

use crate::dom::Node;
use crate::dom::text_content::gather_text;

/// Текст поддерева — нужен формам (`<textarea>`, `<option>`).
pub fn gather_text_public(nodes: &[Node], out: &mut String) {
    gather_text(nodes, out)
}
