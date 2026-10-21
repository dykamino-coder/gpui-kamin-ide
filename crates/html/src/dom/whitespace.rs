//! Текстовые узлы только из пробелов в стилизованном дереве документа.

use crate::dom::Node;
use crate::text::text_box::blank_text;

pub(crate) fn is_blank(n: &Node) -> bool {
    matches!(n, Node::Text(t) if blank_text(t))
}
