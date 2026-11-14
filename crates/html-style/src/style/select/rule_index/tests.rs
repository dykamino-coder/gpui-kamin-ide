//! Регрессии полноты индекса и порядка кандидатов.

use super::*;
#[test]
fn index_keeps_universal_and_necessary_subject_tokens() {
    for (raw, expected) in [
        (".a .b", Some((1, "b"))),
        ("#id.x", Some((0, "id"))),
        ("DIV[x]", Some((2, "div"))),
        ("[x]", Some((3, "x"))),
        (":is(.a,.b)", None),
    ] {
        let sel = Selector::parse(raw).unwrap();
        assert_eq!(token(&sel), expected);
    }
}

#[test]
fn repeated_compounds_fit_one_stylesheet_preparation_budget() {
    use crate::budget::{BudgetKind, DocumentBudget, DocumentLimits};
    use crate::dom::tree_index::TreeIndex;
    let dom = crate::dom::xml::xml_dom(&format!(
        "<r>{}</r>",
        "<i class='left'/><b class='right'/>".repeat(64)
    ))
    .unwrap();
    let doc = crate::values::DocumentValues::detached(Default::default());
    let owner = DocumentBudget::new(DocumentLimits::default(), Default::default()).unwrap();
    let revision = owner.revision();
    doc.admit_revision(revision.clone(), None);
    let tree = TreeIndex::new(doc, true);
    let rules: Vec<_> = (0..64)
        .map(|order| Rule {
            sel: Selector::parse(".left ~ .right").unwrap(),
            decls: Default::default(),
            order,
            origin: crate::style::css::Origin::Author,
            layer: vec![u32::MAX],
            link: crate::style::css::LinkMatch::Any,
        })
        .collect();
    let index = RuleIndex::new(&rules, &tree.admission()).unwrap();
    let before = revision.used(BudgetKind::TraversalVisits);
    index
        .prepare_bulk(&dom.document, &rules, &tree, false)
        .unwrap();
    let work = revision.used(BudgetKind::TraversalVisits) - before;
    assert!(
        work < 20_000,
        "repeated local matches must be shared: {work} visits"
    );
    let root = dom.document.children.borrow()[0].clone();
    let right = root.children.borrow()[1].clone();
    assert_eq!(
        index
            .bulk
            .borrow()
            .as_ref()
            .unwrap()
            .candidates(&crate::dom::tree_index::NodeKey::of(&right)),
        (0..64).collect::<Vec<_>>()
    );
}
