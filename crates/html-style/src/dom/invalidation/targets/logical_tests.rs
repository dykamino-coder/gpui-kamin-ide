//! Logical selector dependencies stay local or retain the full fallback.

use super::*;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::RcDom;
use std::rc::Rc;

fn changed() -> Changed {
    Changed {
        elements: vec![1],
        classes: vec!["alt".into()],
        attrs: vec!["class".into()],
    }
}

fn nodes() -> (Handle, Handle, Handle, Handle) {
    let dom = html5ever::parse_document(RcDom::default(), Default::default()).one(
        "<html><body><section><figure><p></p></figure></section><aside><p></p></aside></body></html>",
    );
    let mut stack = vec![dom.document];
    let mut found = Vec::new();
    while let Some(node) = stack.pop() {
        stack.extend(node.children.borrow().iter().rev().cloned());
        if let NodeData::Element { name, .. } = &node.data
            && matches!(&*name.local, "html" | "section" | "p")
        {
            found.push(node);
        }
    }
    assert_eq!(found.len(), 4);
    (
        found[0].clone(),
        found[1].clone(),
        found[2].clone(),
        found[3].clone(),
    )
}

#[test]
fn local_logical_arguments_restyle_only_the_mutated_subject() {
    let (html, _, _, _) = nodes();
    for selector in [
        ".content figure:not([class])",
        ".content figure:not([class]):not(:first-child)",
        "figure:is(.alt,[class])",
        "figure:where(:not([class]),.other)",
        "figure:not([class]):nth-child(2n)",
    ] {
        let css = format!("{selector}{{background:red}}");
        let rules = crate::style::css::parse_stylesheet(&css).unwrap();
        let targets = targets(&rules, &changed(), std::slice::from_ref(&html)).unwrap();
        assert_eq!(targets.len(), 1, "{selector}");
        assert!(Rc::ptr_eq(&targets[0], &html), "{selector}");
    }
}

#[test]
fn cached_logical_ancestor_features_follow_each_mutated_root() {
    let (html, section, inside, outside) = nodes();
    let rules =
        crate::style::css::parse_stylesheet("figure:not([class]) p{background:red}").unwrap();
    let features = features(&rules, &changed()).unwrap();
    let all = features.targets(std::slice::from_ref(&html)).unwrap();
    assert_eq!(all.len(), 3);
    assert!(all.iter().any(|h| Rc::ptr_eq(h, &inside)));
    assert!(all.iter().any(|h| Rc::ptr_eq(h, &outside)));
    let local = features.targets(std::slice::from_ref(&section)).unwrap();
    assert_eq!(local.len(), 2);
    assert!(local.iter().any(|h| Rc::ptr_eq(h, &inside)));
    assert!(!local.iter().any(|h| Rc::ptr_eq(h, &outside)));
}

#[test]
fn cross_element_or_state_arguments_keep_the_full_fallback() {
    for selector in [
        "p:not(.alt p)",
        "p:is(.alt + p)",
        "p:has(.alt)",
        "p:not(:has(.alt))",
        "p:nth-child(2n of .alt)",
        ".alt + p:not([class])",
        ".alt::before",
        "p:not([class]):checked",
    ] {
        let css = format!("{selector}{{background:red}}");
        let rules = crate::style::css::parse_stylesheet(&css).unwrap();
        assert!(features(&rules, &changed()).is_none(), "{selector}");
    }
}
