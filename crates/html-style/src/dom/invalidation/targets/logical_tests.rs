//! Logical selector dependencies stay local or retain the full fallback.

use super::*;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{NodeData, RcDom};
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
        let targets = targets(&rules, &changed(), std::slice::from_ref(&html));
        assert_eq!(targets.len(), 1, "{selector}");
        assert!(Rc::ptr_eq(&targets[0], &html), "{selector}");
    }
}

#[test]
fn cached_logical_ancestor_features_follow_each_mutated_root() {
    let (html, section, inside, outside) = nodes();
    let rules = crate::style::css::parse_stylesheet(":not([class]) p{background:red}").unwrap();
    let features = features(&rules, &changed());
    let all = features.targets(std::slice::from_ref(&html));
    assert_eq!(all.len(), 3);
    assert!(all.iter().any(|h| Rc::ptr_eq(h, &inside)));
    assert!(all.iter().any(|h| Rc::ptr_eq(h, &outside)));
    let local = features.targets(std::slice::from_ref(&section));
    assert_eq!(local.len(), 2);
    assert!(local.iter().any(|h| Rc::ptr_eq(h, &inside)));
    assert!(!local.iter().any(|h| Rc::ptr_eq(h, &outside)));
}

/// The tags of `targets` in tree order of the found list.
fn tags(targets: &[Handle]) -> Vec<String> {
    targets
        .iter()
        .map(|h| match &h.data {
            NodeData::Element { name, .. } => name.local.to_string(),
            _ => String::new(),
        })
        .collect()
}

#[test]
fn cross_element_arguments_follow_their_combinators_instead_of_refusing() {
    // The class changes on `section`; `aside` is its next sibling.
    // The root keeps the tree (and the parents of section) alive.
    let (_root, section, _, _) = nodes();
    let cases = [
        // An argument with a descendant combinator: descendants with the subject.
        ("p:not(.alt p)", vec!["section", "p"]),
        ("p:is(.alt *)", vec!["section", "p"]),
        // Sibling combinators, also inside an argument.
        ("aside:is(.alt + aside)", vec!["section", "aside"]),
        (".alt ~ aside p", vec!["section", "p"]),
        // `:nth-child(of S)` counts siblings, `:has()` matches anchors above.
        ("aside:nth-child(2n of .alt)", vec!["section", "aside"]),
        ("body:has(.alt)", vec!["section", "body"]),
        ("html:not(:has(> body > .alt))", vec!["section", "html"]),
        // State and pseudo-elements depend on the element itself.
        (".alt::before", vec!["section"]),
        (".alt:hover", vec!["section"]),
        ("p:not([class]):checked", vec!["section"]),
    ];
    for (selector, want) in cases {
        let css = format!("{selector}{{background:red}}");
        let rules = crate::style::css::parse_stylesheet(&css).unwrap();
        let found = targets(&rules, &changed(), std::slice::from_ref(&section));
        let mut got = tags(&found);
        got.sort();
        let mut want: Vec<String> = want.into_iter().map(String::from).collect();
        want.sort();
        assert_eq!(got, want, "{selector}");
    }
}

#[test]
fn a_name_in_an_argument_not_parsed_here_restyles_the_whole_document() {
    // Every `p` of the document, also the one outside the mutated `section`.
    let (_root, section, _, outside) = nodes();
    let rules = crate::style::css::parse_stylesheet("p:host(.alt){color:red}").unwrap();
    let found = targets(&rules, &changed(), std::slice::from_ref(&section));
    assert_eq!(tags(&found), ["section", "p", "p"]);
    assert!(found.iter().any(|h| Rc::ptr_eq(h, &outside)));
}

#[test]
fn only_sibling_compounds_read_a_parents_census() {
    let reads = |css: &str| {
        let rules = crate::style::css::parse_stylesheet(css).unwrap();
        features(&rules, &changed()).reads_siblings()
    };
    for css in [
        "html.alt [data-deep]{background:red}",
        ".alt > p{color:red}",
        "p.alt{color:red}",
        "p:is(.alt){color:red}",
    ] {
        assert!(!reads(css), "{css}");
    }
    for css in [
        ".alt + p{color:red}",
        ".alt ~ p{color:red}",
        "p:nth-child(2 of .alt){color:red}",
        "div:has(.alt){color:red}",
    ] {
        assert!(reads(css), "{css}");
    }
}

#[test]
fn escaped_names_in_arguments_are_read_by_their_decoded_names() {
    // Utility sheets escape names in arguments (`.space-y-0\.5`): an argument
    // that reads no changed name stays local, one whose decoded name is a
    // changed one follows it, whether its selectors parse here or not.
    let (_root, section, _, _) = nodes();
    let cases = [
        (
            r":where(.space-y-0\.5>:not(:last-child)){margin:0}",
            vec!["section"],
        ),
        (r":is(.\*\:flex>*){display:flex}", vec!["section"]),
        (r"p:not(.\61 lt p){color:red}", vec!["section", "p"]),
        (r"p:host(.\61lt){color:red}", vec!["section", "p", "p"]),
        (r"p:host(.x\:y){color:red}", vec!["section"]),
    ];
    for (css, want) in cases {
        let rules = crate::style::css::parse_stylesheet(css).unwrap();
        let found = targets(&rules, &changed(), std::slice::from_ref(&section));
        let mut got = tags(&found);
        got.sort();
        let mut want: Vec<String> = want.into_iter().map(String::from).collect();
        want.sort();
        assert_eq!(got, want, "{css}");
    }
}
#[test]
fn a_compound_the_mutated_element_cannot_match_reaches_nothing() {
    // The class changes on `section`: a compound whose type, other classes or
    // `:is()` alternatives `section` lacks matches it neither before nor
    // after, so its rule restyles nothing past the mutated element.
    let (_root, section, _, _) = nodes();
    let cases = [
        (
            ":is(ol,ul):where(:not([class])) p{margin:0}",
            vec!["section"],
        ),
        ("blockquote:not([class])>*+*{margin:0}", vec!["section"]),
        ("figure:not([class]) p{margin:0}", vec!["section"]),
        ("pre.alt code{margin:0}", vec!["section"]),
        ("section.alt.other p{margin:0}", vec!["section"]),
        ("section#x.alt p{margin:0}", vec!["section"]),
        // The compound `section` can match: its rules still follow.
        (
            ":is(ol,section):where(:not([class])) p{margin:0}",
            vec!["section", "p"],
        ),
        ("section.alt p{margin:0}", vec!["section", "p"]),
        (":not(pre).alt p{margin:0}", vec!["section", "p"]),
        ("figure:has(~ .alt) p{margin:0}", vec!["section", "p", "p"]),
    ];
    for (css, want) in cases {
        let rules = crate::style::css::parse_stylesheet(css).unwrap();
        let found = targets(&rules, &changed(), std::slice::from_ref(&section));
        let mut got = tags(&found);
        got.sort();
        let mut want: Vec<String> = want.into_iter().map(String::from).collect();
        want.sort();
        assert_eq!(got, want, "{css}");
    }
}
