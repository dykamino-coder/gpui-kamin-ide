//! Перепись детей: порядки элементов, точные ключи типов и большие группы.

use super::{Spot, census_of};
use crate::dom::tree_index::TreeIndex;

fn values(spot: &Spot) -> (usize, usize, usize, usize) {
    (spot.index, spot.total, spot.of_type, spot.of_type_total)
}

#[test]
fn census_preserves_local_name_keys_and_non_element_slots() {
    let dom = crate::dom::xml::xml_dom(
        "<root>text<A/><a/><!--comment--><A xmlns='urn:other'/><a/><B/></root>",
    );
    let root = dom.document.children.borrow()[0].clone();
    let children = root.children.borrow();
    let (spots, all) = census_of(&children, &TreeIndex::default());
    let actual: Vec<_> = spots.iter().map(values).collect();
    assert_eq!(
        actual,
        vec![
            (0, 0, 0, 0),
            (1, 5, 1, 2),
            (2, 5, 1, 2),
            (0, 0, 0, 0),
            (3, 5, 2, 2),
            (4, 5, 2, 2),
            (5, 5, 1, 1),
        ]
    );
    assert_eq!(
        all.iter().map(|a| values(&a.spot)).collect::<Vec<_>>(),
        actual.into_iter().filter(|s| s.0 != 0).collect::<Vec<_>>()
    );
}

#[test]
fn census_large_sibling_group_has_forward_and_reverse_positions() {
    let dom = crate::dom::xml::xml_dom(&format!("<root>{}</root>", "<a/><b/>".repeat(4096)));
    let root = dom.document.children.borrow()[0].clone();
    let (spots, all) = census_of(&root.children.borrow(), &TreeIndex::default());
    assert_eq!(all.len(), 8192);
    for (i, spot) in spots.iter().enumerate() {
        assert_eq!(values(spot), (i + 1, 8192, i / 2 + 1, 4096));
        assert_eq!(spot.of_type_total + 1 - spot.of_type, 4096 - i / 2);
    }
}

#[test]
fn census_empty_and_non_element_groups_have_no_elements() {
    let dom = crate::dom::xml::xml_dom("<root>text<!--comment--></root>");
    let root = dom.document.children.borrow()[0].clone();
    for children in [&[][..], &root.children.borrow()[..]] {
        let (spots, all) = census_of(children, &TreeIndex::default());
        assert!(all.is_empty());
        assert!(spots.iter().all(|s| values(s) == (0, 0, 0, 0)));
    }
}
