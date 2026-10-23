//! Видимость потомков стилизованной коробки.

use crate::dom::Node;

/// Объявлен ли где-то в поддереве СВОЙ `visibility: visible`.
///
/// По §11.2 потомок скрытого элемента виден, если объявил видимость сам.
/// Читается собственное значение узла, до слияния: `Some(false)` приходит
/// только из авторского CSS — прочие места ставят лишь `Some(true)`.
pub(super) fn shows_inside(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Text(_) => false,
        Node::Element(e) => e.style.hidden == Some(false) || shows_inside(&e.children),
    })
}
