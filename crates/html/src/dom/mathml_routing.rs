//! Оставляет неполные MathML-контексты на прежнем пути каскада и layout.
//!
//! Core §3 требует общего базового контекста baseline. Выбираются целые математические
//! деревья вместе с их UA-умолчаниями, а не смесь несовместимых алгоритмов боксов.
use super::mathml::is_mathml;
use markup5ever_rcdom::{Handle, NodeData};

const CONSTRUCTS: &[&str] = &[
    "mspace",
    "mpadded",
    "mfrac",
    "mstyle",
    "mphantom",
    "msub",
    "msup",
    "msubsup",
    "munder",
    "mover",
    "munderover",
];

pub(super) fn enabled(handle: &Handle) -> bool {
    if !is_mathml(handle) {
        return false;
    }
    let mut root = handle.clone();
    loop {
        let parent = root.parent.take();
        let next = parent.as_ref().and_then(std::rc::Weak::upgrade);
        root.parent.set(parent);
        match next.filter(is_mathml) {
            Some(parent) => root = parent,
            None => break,
        }
    }
    supported(&root)
}

fn supported(handle: &Handle) -> bool {
    let NodeData::Element { name, .. } = &handle.data else {
        return true;
    };
    if !is_mathml(handle) {
        return false;
    }
    let tag = name.local.as_ref();
    if !CONSTRUCTS.contains(&tag) && !matches!(tag, "math" | "mrow") {
        return false;
    }
    // Only the Core §3.3.2 row fallback is ready. Fraction geometry still
    // needs margin-box metrics and correct nested rule placement.
    if tag == "mfrac"
        && handle
            .children
            .borrow()
            .iter()
            .filter(|child| matches!(child.data, NodeData::Element { .. }))
            .count()
            == 2
    {
        return false;
    }
    // Core §3.2.5: mspace is empty even when the DOM contains children.
    if tag == "mspace" {
        return true;
    }
    // Empty inferred rows share the context of an explicitly empty mspace.
    // Text metrics and operator stretching remain on the original path.
    handle.children.borrow().iter().all(supported)
}

#[cfg(test)]
mod tests {
    use crate::dom::{Element, Node, parse};

    fn find(nodes: &[Node], id: &str) -> Option<Element> {
        nodes.iter().find_map(|node| match node {
            Node::Element(e) if e.attr("id") == Some(id) => Some(e.clone()),
            Node::Element(e) => find(&e.children, id),
            _ => None,
        })
    }

    #[test]
    fn unsupported_siblings_keep_the_whole_tree_on_the_original_path() {
        for unsupported in ["mtable", "mmultiscripts", "mo", "mi", "mtext"] {
            let nodes = parse(
                &format!(
                    "<math id=root><mspace id=space width='20px' /><{unsupported}></{unsupported}></math>"
                ),
                "",
            );
            for id in ["root", "space"] {
                let e = find(&nodes, id).expect("MathML element");
                assert!(e.style.math_layout.is_none(), "{unsupported}: {id}");
                assert!(!e.style.math_font_size, "{unsupported}: {id}");
            }
        }
    }

    #[test]
    fn ignored_children_do_not_disable_a_space_context() {
        let nodes = parse(
            "<math id=root><mspace id=space><mtable></mtable></mspace></math>",
            "",
        );
        for id in ["root", "space"] {
            let e = find(&nodes, id).expect("MathML element");
            assert!(e.style.math_layout.is_some());
            assert!(e.style.math_font_size);
        }
    }

    #[test]
    fn html_display_math_stays_on_the_original_path() {
        let nodes = parse("<span id=html style='display: inline math'>x</span>", "");
        let e = find(&nodes, "html").expect("HTML element");
        assert!(e.style.math_layout.is_none());
        assert_eq!(e.style.display, None);
    }

    #[test]
    fn empty_and_padded_rows_share_the_space_context() {
        for content in ["", "<mrow></mrow>", "<mpadded width='5%'></mpadded>"] {
            let nodes = parse(&format!("<math id=root>{content}</math>"), "");
            assert!(find(&nodes, "root").unwrap().style.math_layout.is_some());
        }
    }
}
