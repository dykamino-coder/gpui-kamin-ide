//! Пересборка полос таблицы после распорки использует обновлённое дерево.
use super::rebuild;
use crate::dom::{Element, Node};
use crate::layout::fragment::types::{Repeat, StackAxis};
use crate::layout::multicol::deferred::Recipe;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use gpui::IntoElement;
use std::{cell::RefCell, rc::Rc};

fn section(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "section" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("section")
}

#[test]
fn grown_table_rebuilds_repeated_bands_with_the_main_source() {
    let tree = crate::dom::parse(
        "<section><table><tr><td style='height:130px;break-after:column'></td></tr><tfoot style='break-inside:avoid'><tr><td style='height:20px'></td></tr></tfoot></table></section>",
        "",
    );
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let recipe = Recipe::new(
        section(&tree),
        &BoxStyle::default(),
        &opts,
        4,
        0.0,
        Some(100.0),
        StackAxis::Horizontal,
        None,
    )
    .expect("table recipe");
    let mut children = recipe.children(100.0);
    let child = &mut children[0];
    child.repeat = Some(Repeat {
        head: Some((0.0, 10.0)),
        foot: Some((150.0, 20.0)),
        geom: Default::default(),
        head_els: (0..3).map(|_| gpui::div().into_any_element()).collect(),
        foot_els: (0..4).map(|_| gpui::div().into_any_element()).collect(),
    });
    let seen = Rc::new(RefCell::new(Vec::new()));
    let capture = seen.clone();
    let regrow = child.regrow.as_mut().expect("measured table growth");
    regrow.dom.style.used_mut().height = Some(Len::Px(370.0));
    regrow.build = Rc::new(move |dom, part, band| {
        capture
            .borrow_mut()
            .push((dom.style.used().height, part, band));
        gpui::div().into_any_element()
    });
    let spare = child.frags.len();
    rebuild(child);
    let seen = seen.borrow();
    assert!(seen.iter().all(|item| item.0 == Some(Len::Px(370.0))));
    assert_eq!(seen.iter().filter(|item| !item.2).count(), spare + 1);
    assert_eq!(
        seen.iter()
            .filter(|item| item.2)
            .map(|item| item.1)
            .collect::<Vec<_>>(),
        [1, 2, 3, 0, 1, 2, 3]
    );
}
