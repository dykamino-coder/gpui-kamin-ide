//! Создание анонимной коробки для фрагмента разорванного строчного.

use crate::dom::Node;
use crate::layout::table::anon::anon_element;

#[allow(clippy::too_many_arguments)]
pub(crate) fn flush_inline(
    piece: &mut Vec<Node>,
    out: &mut Vec<Node>,
    hosts: &mut Vec<usize>,
    first: bool,
    last: bool,
    e: &crate::dom::Element,
    ltr: bool,
    has_edge: impl Fn(usize) -> bool,
) {
    // Кусок из одних схлопываемых пробелов коробки не создаёт —
    // иначе он рисовал бы фон и рамку строчного на пустом месте.
    let blank = piece.iter().all(|n| match n {
        Node::Text(t) => t.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')),
        Node::Element(_) => false,
    });
    if piece.is_empty() || blank {
        piece.clear();
        // …кроме первого и последнего куска со своим краем: строка с
        // пустой строчной коробкой, у которой есть рамка, отбивка или
        // поле по строчной оси, не пуста (CSS 2.1 §9.4.2), а разрыв
        // §9.2.1.1 оставляет начальный край первому куску, конечный —
        // последнему (`block-in-inline-whitespace-001a`: синяя черта
        // `border-left` над первым блоком и `border-right` под вторым).
        let drop = if first && ltr && has_edge(3) {
            Some(1)
        } else if last && ltr && has_edge(1) {
            Some(3)
        } else {
            None
        };
        if let Some(drop) = drop {
            let mut host = e.clone();
            host.children = Vec::new();
            host.style.border_visible[drop] = Some(false);
            if drop == 1 {
                host.style.border_width.right = None;
                host.style.padding.right = None;
                host.style.margin.right = None;
            } else {
                host.style.border_width.left = None;
                host.style.padding.left = None;
                host.style.margin.left = None;
            }
            hosts.push(out.len());
            out.push(Node::Element(anon_element(
                "anon-block",
                vec![Node::Element(host)],
            )));
        }
        return;
    }
    let mut host = e.clone();
    host.children = std::mem::take(piece);
    hosts.push(out.len());
    out.push(Node::Element(anon_element(
        "anon-block",
        vec![Node::Element(host)],
    )));
}
