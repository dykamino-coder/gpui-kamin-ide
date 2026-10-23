//! Оставшаяся часть row-gap перед строкой после усечения разрывом.
use super::*;

#[test]
fn carried_gap_excludes_fragment_growth_and_tracks_truncation() {
    for (by, remaining) in [(-10.0, 0.0), (-6.0, 4.0), (0.0, 10.0), (20.0, 10.0)] {
        let mut tree = crate::dom::parse(
            "<div style='display:flex;flex-wrap:wrap;row-gap:10px'><div style='margin-top:3px;height:50px'></div></div>",
            "",
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
        let struts = HashMap::from([(
            item.node_id,
            Strut {
                by,
                gap: 10.0,
                size: 50.0,
            },
        )]);
        assert!(apply(parent, &struts));
        let Node::Element(item) = &parent.children[0] else {
            panic!("item")
        };
        assert_eq!(item.marks.carried_row_gap, remaining, "growth {by}");
        assert_eq!(item.style.margin.top, Some(Len::Px(13.0 + by)));
        assert_eq!(parent.style.gap.unwrap().0, Some(Len::Px(0.0)));
    }
}
