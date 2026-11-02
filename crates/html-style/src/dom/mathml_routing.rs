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

pub(super) fn enabled(handle: &Handle, vcx: crate::values::ValueCx<'_>) -> bool {
    let admission = crate::budget::admission::Admission::new(vcx.doc.revision_budget());
    match enabled_in(handle, &admission) {
        Ok(enabled) => enabled,
        Err(error) => {
            let failure = crate::dom::outcome::DocumentFailure::from(error);
            vcx.doc.revision_budget().reject(failure);
            vcx.doc
                .record_observed(crate::diagnostics::Diagnostic::from(&failure));
            false
        }
    }
}

fn enabled_in(
    handle: &Handle,
    admission: &crate::budget::admission::Admission,
) -> Result<bool, crate::budget::admission::Error> {
    if !is_mathml(handle) {
        return Ok(false);
    }
    let mut root = handle.clone();
    loop {
        admission.work(1)?;
        let parent = root.parent.take();
        let next = parent.as_ref().and_then(|parent| parent.upgrade());
        root.parent.set(parent);
        match next.filter(is_mathml) {
            Some(parent) => root = parent,
            None => break,
        }
    }
    supported(&root, admission)
}

fn supported(
    handle: &Handle,
    admission: &crate::budget::admission::Admission,
) -> Result<bool, crate::budget::admission::Error> {
    let mut stack = crate::budget::admission::Buffer::<(Handle, usize)>::default();
    let mut next = Some(handle.clone());
    while next.is_some() || !stack.is_empty() {
        admission.work(1)?;
        let handle = if let Some(handle) = next.take() {
            handle
        } else {
            let (parent, index) = stack.last_mut().expect("MathML frame");
            let child = parent.children.borrow().get(*index).cloned();
            if let Some(child) = child {
                *index += 1;
                child
            } else {
                stack.pop();
                continue;
            }
        };
        let NodeData::Element { name, .. } = &handle.data else {
            continue;
        };
        if !is_mathml(&handle) {
            return Ok(false);
        }
        let tag = name.local.as_ref();
        if !CONSTRUCTS.contains(&tag) && !matches!(tag, "math" | "mrow") {
            return Ok(false);
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
            return Ok(false);
        }
        // Core §3.2.5: mspace is empty even when the DOM contains children.
        if tag == "mspace" {
            continue;
        }
        // Empty inferred rows share the context of an explicitly empty mspace.
        // Text metrics and operator stretching remain on the original path.
        stack.push((handle, 0), admission)?;
    }
    Ok(true)
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
                assert!(
                    e.style.cascaded().reset_svg_math.math_layout.is_none(),
                    "{unsupported}: {id}"
                );
                assert!(
                    !e.style.cascaded().reset_svg_math.math_font_size,
                    "{unsupported}: {id}"
                );
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
            assert!(e.style.cascaded().reset_svg_math.math_layout.is_some());
            assert!(e.style.cascaded().reset_svg_math.math_font_size);
        }
    }

    #[test]
    fn html_display_math_stays_on_the_original_path() {
        let nodes = parse("<span id=html style='display: inline math'>x</span>", "");
        let e = find(&nodes, "html").expect("HTML element");
        assert!(e.style.cascaded().reset_svg_math.math_layout.is_none());
        assert_eq!(e.style.cascaded().reset_box_layout.display, None);
    }

    #[test]
    fn empty_and_padded_rows_share_the_space_context() {
        for content in ["", "<mrow></mrow>", "<mpadded width='5%'></mpadded>"] {
            let nodes = parse(&format!("<math id=root>{content}</math>"), "");
            assert!(
                find(&nodes, "root")
                    .unwrap()
                    .style
                    .cascaded()
                    .reset_svg_math
                    .math_layout
                    .is_some()
            );
        }
    }
}
