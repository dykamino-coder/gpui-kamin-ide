//! Распорка элемента ряда flex — его верхнее поле; заданный row-gap не переписывается.
use super::*;

#[test]
fn row_item_strut_moves_its_margin_and_keeps_the_gap() {
    for by in [-6.0, 0.0, 20.0] {
        let mut tree = crate::dom::parse(
            "<div style='display:flex;flex-wrap:wrap;row-gap:10px'><div style='margin-top:3px;height:50px'></div></div>",
            "",
        );
        crate::style::cascade::materialize::materialize(
            &tree.nodes,
            &std::rc::Rc::new(crate::style::computed::Computed::default()),
            tree.values(),
        );
        fn div(node: &mut Node) -> Option<&mut Element> {
            let Node::Element(e) = node else { return None };
            if e.tag == "div" {
                return Some(e);
            }
            e.children.iter_mut().find_map(div)
        }
        let parent = tree.iter_mut().find_map(div).expect("flex root");
        let Node::Element(item) = &parent.children[0] else {
            panic!("item")
        };
        let struts = HashMap::from([(item.node_id, Strut { by, size: 50.0 })]);
        assert!(apply(parent, &struts));
        let Node::Element(item) = &parent.children[0] else {
            panic!("item")
        };
        assert_eq!(item.style.used().margin.top, Some(Len::Px(3.0 + by)));
        assert_eq!(parent.style.used().gap.unwrap().0, Some(Len::Px(10.0)));
        assert_eq!(item.style.computed().margin.top, Some(Len::Px(3.0)));
    }
}
