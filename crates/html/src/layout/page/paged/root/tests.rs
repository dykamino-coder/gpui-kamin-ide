//! Границы общего пробега обтекания при принудительном разрыве страницы.
use super::*;
use crate::dom::Element;

fn body(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "body" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("body")
}

#[test]
fn forced_float_after_ends_its_shared_inline_run() {
    for forced in [false, true] {
        let tree = crate::dom::parse(
            &format!(
                "<body><br><div style='float:left;{}'>one</div><div style='clear:both'>two</div></body>",
                if forced { "break-after:page" } else { "" }
            ),
            "",
        );
        let floated = |n: &Node| matches!(n, Node::Element(e) if e.style.used().float.is_some_and(|f| f != 0));
        let mut groups = Vec::new();
        group_runs(
            body(&tree).children.clone(),
            |n| matches!(n, Node::Element(e) if e.tag == "br"),
            floated,
            &mut groups,
            tree.values(),
        );
        assert_eq!(groups.len(), if forced { 2 } else { 1 });
    }
}

#[test]
fn forced_boundary_does_not_carry_float_run_into_next_page() {
    let tree = crate::dom::parse(
        "<body><br><div style='float:left;break-after:page'>one</div><span>two</span><div>three</div></body>",
        "",
    );
    let mut groups = Vec::new();
    group_runs(
        body(&tree).children.clone(),
        |n| matches!(n, Node::Element(e) if matches!(e.tag.as_str(), "br" | "span")),
        |n| matches!(n, Node::Element(e) if e.style.used().float.is_some_and(|f| f != 0)),
        &mut groups,
        tree.values(),
    );
    assert_eq!(groups.len(), 3);
}
