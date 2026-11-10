//! Tests of the restyle invalidation of attribute mutations (`Invalidation`).

use super::*;

fn alt_on(id: u64) -> Changed {
    Changed {
        elements: vec![id],
        classes: vec!["alt".into()],
        attrs: vec!["class".into()],
    }
}

fn restyled(css: &str, changed: Changed, tree: &[(&str, &str, u64)]) -> Option<Vec<u64>> {
    let rules = crate::style::css::parse_stylesheet(css).unwrap();
    let mut invalidation = Invalidation::new(changed);
    invalidation.prepare(&rules, false);
    for &(tag, class, id) in tree {
        invalidation.visit(tag, &[("class".into(), class.into())], id);
    }
    invalidation.finish()
}

const TREE: [(&str, &str, u64); 4] = [
    ("p", "t1", 1),
    ("p", "t2", 2),
    ("body", "", 3),
    ("html", "alt", 4),
];

#[test]
fn a_class_restyles_its_element_and_the_subjects_of_the_rules_that_read_it() {
    let css = "html,body{color:black} html.alt body{color:#111} p{margin:0} .t1{font-style:italic}";
    assert_eq!(restyled(css, alt_on(4), &TREE), Some(vec![4, 3]));
    let unread = "html,body{color:black} .t1{font-style:italic}";
    assert_eq!(restyled(unread, alt_on(4), &TREE), Some(vec![4]));
    let paragraphs = ".alt .t2{color:red} .alt + p{color:blue}";
    assert_eq!(restyled(paragraphs, alt_on(4), &TREE), Some(vec![4, 1, 2]));
}

#[test]
fn arguments_and_attribute_selectors_read_their_names() {
    for css in [
        "p:is(.alt *){color:red}",
        "p:not(.alt p){color:red}",
        "p:has(.alt){color:red}",
        "p[class~=alt]{color:red}",
        "p:nth-child(2 of .alt){color:red}",
    ] {
        assert_eq!(
            restyled(css, alt_on(4), &TREE),
            Some(vec![4, 1, 2]),
            "{css}"
        );
    }
}

#[test]
fn a_dependent_rule_without_subject_features_restyles_anything() {
    for css in [
        ".alt *{color:red}",
        ".alt ::before{content:'x'}",
        ".alt :is(p){color:red}",
    ] {
        assert_eq!(restyled(css, alt_on(4), &TREE), None, "{css}");
    }
    let rules = crate::style::css::parse_stylesheet("html.alt body{color:red}").unwrap();
    let mut shadowed = Invalidation::new(alt_on(4));
    shadowed.prepare(&rules, true);
    assert_eq!(shadowed.finish(), None, "shadow trees are not followed");
    assert_eq!(Invalidation::new(alt_on(4)).finish(), None, "rules unknown");
}

#[test]
fn many_restyled_elements_give_up_the_set() {
    let tree: Vec<_> = (1..=LIMIT as u64 + 1).map(|id| ("p", "", id)).collect();
    assert_eq!(restyled(".alt p{color:red}", alt_on(99), &tree), None);
    assert_eq!(
        restyled(".alt p{color:red}", alt_on(99), &tree[..LIMIT]).map(|v| v.len()),
        Some(LIMIT + 1)
    );
}
