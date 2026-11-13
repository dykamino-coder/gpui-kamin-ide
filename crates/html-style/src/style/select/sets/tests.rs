//! Дифференциальные проверки множеств против канонического сопоставления.

use super::*;
use crate::style::select::has::{mark_has, parse_has_arg};

#[test]
fn relations_and_compounds_equal_element_major_matching_across_word_boundaries() {
    let xml = format!(
        "<html><r class='a'>{}</r><r><b/></r></html>",
        "<a class='x'/><!-- gap --><b data-v='YES'><i class='leaf'/><i/></b><c/>".repeat(37)
    );
    let dom = crate::dom::xml::xml_dom(&xml).unwrap();
    let mut tree = TreeIndex::new(
        crate::values::DocumentValues::detached(Default::default()),
        true,
    );
    mark_has(
        &dom.document,
        &[parse_has_arg(".leaf").unwrap()],
        &mut vec![],
        &mut tree,
    )
    .unwrap();
    let scope = Scope::new(&dom.document, &[], &tree).unwrap();
    for raw in [
        ".a .leaf",
        "a + b > i",
        "a ~ c",
        "b:has(.leaf) i:nth-child(2)",
        "b[data-v='yes' i] > i:first-of-type",
        "html:root > r > c:last-child",
        "b:empty + c",
        "#absent ~ b",
        "r > a + b > i + i",
        "a + b ~ c",
        "b:has(.absent) i",
    ] {
        let sel = Selector::parse(raw).unwrap_or_else(|| panic!("selector fixture: {raw}"));
        assert!(eligible(&sel), "{raw}");
        let actual = matches(&scope, &sel, &tree).unwrap();
        for i in 0..scope.records.len() {
            assert_eq!(
                actual.contains(i),
                scope.match_at(&sel, i, &tree).unwrap(),
                "{raw}, node {i}"
            );
        }
    }
}

#[test]
fn contextual_and_layer_pseudos_keep_the_canonical_path() {
    for raw in [
        "a:visited i",
        "a:hover i",
        "a i::before",
        ":is(a,b) i",
        "a:dir(rtl) i",
        "a:nth-child(2 of .x) i",
        "a:has(x):has(y) i",
        "a:lang(en) i",
    ] {
        assert!(!eligible(&Selector::parse(raw).unwrap()), "{raw}");
    }
}

#[test]
fn relative_sets_equal_anchor_overlay_for_mixed_chains() {
    let xml = format!(
        "<r>{}</r>",
        "<a class='a'/><b><i/><i class='x'/></b><c/>".repeat(24)
    );
    let dom = crate::dom::xml::xml_dom(&xml).unwrap();
    let tree = TreeIndex::new(
        crate::values::DocumentValues::detached(Default::default()),
        true,
    );
    let scope = Scope::new(&dom.document, &[], &tree).unwrap();
    for raw in [
        "~ b > .x",
        "+ b > i + .x",
        "> a ~ b > i:nth-child(2)",
        "b > i + .x",
        "~ .absent ~ c",
        "+ c",
        "~ b + c",
    ] {
        let arg = parse_has_arg(raw).unwrap();
        let sel = &arg.parts[0].1;
        let actual = anchors(&scope, sel, &tree).unwrap();
        for (anchor, record) in scope.records.iter().enumerate() {
            let _overlay = tree
                .matching
                .anchor(crate::dom::tree_index::NodeKey::of(&record.handle));
            let expected = (0..scope.records.len()).any(|i| scope.match_at(sel, i, &tree).unwrap());
            assert_eq!(actual.contains(anchor), expected, "{raw}, anchor {anchor}");
        }
    }
    assert!(!tree.matching.has_anchor());
}
