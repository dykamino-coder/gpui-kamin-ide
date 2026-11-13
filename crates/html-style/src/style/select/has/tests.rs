//! Проверки областей и стоимости реляционных селекторов.
use super::*;

fn marked(xml: &str, raw: &str) -> Vec<String> {
    let dom = crate::dom::xml::xml_dom(xml).unwrap();
    let mut tree = TreeIndex::new(
        crate::values::DocumentValues::detached(Default::default()),
        true,
    );
    let arg = parse_has_arg(raw).unwrap();
    mark_has(&dom.document, &[arg], &mut vec![], &mut tree).unwrap();
    assert!(!tree.matching.has_anchor());
    let scope = scope::Scope::new(&dom.document, &[], &tree).unwrap();
    scope
        .records
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            let node = &scope.all_at(i, &tree).unwrap()[r.pos];
            (!tree.has_marks_of(&r.handle).is_empty())
                .then(|| node.id.clone())
                .flatten()
        })
        .collect()
}

#[test]
fn relative_relations_keep_their_bounds_and_sibling_identity() {
    let xml = "<root id='r'><a id='a'><x/></a><b id='b'><x/></b><c id='c'/></root>";
    for (arg, expected) in [
        ("x", vec!["r", "a", "b"]),
        ("> x", vec!["a", "b"]),
        ("+ b", vec!["a"]),
        ("~ c", vec!["a", "b"]),
        ("+ b > x", vec!["a"]),
        ("> a + b > x", vec!["r"]),
        ("> x + b", vec![]),
        ("+ c", vec!["b"]),
    ] {
        assert_eq!(marked(xml, arg), expected, "{arg}");
    }
}

#[test]
fn alternative_results_do_not_stop_another_relatives_propagation() {
    assert_eq!(
        marked(
            "<r id='r'><a id='a'><z/><b id='b'><x/></b></a></r>",
            "> z, x"
        ),
        ["r", "a", "b"]
    );
}

#[test]
fn filtered_sibling_positions_use_original_siblings_under_overlay() {
    let xml = "<r id='r'><a id='a'/><b class='keep'/><b class='keep'><x/></b><c/></r>";
    assert_eq!(marked(xml, "+ b:nth-child(1 of .keep) + b > x"), ["a"]);
    assert!(marked(xml, "+ b:nth-child(2 of .keep) + b > x").is_empty());
}

#[test]
fn shadow_host_overlay_is_scoped_and_cannot_claim_siblings() {
    let dom = crate::dom::xml::xml_dom("<host><x/></host>").unwrap();
    let tree = TreeIndex::new(
        crate::values::DocumentValues::detached(Default::default()),
        true,
    );
    let host = dom.document.children.borrow()[0].clone();
    let passport = super::super::ancestor_of(&host, Default::default(), &tree).unwrap();
    let yes = parse_has_arg("> x").unwrap();
    let no = parse_has_arg("+ x").unwrap();
    assert_eq!(
        shadow_marks(&host, &[yes, no], &passport, &tree).unwrap(),
        [has_id("> x")]
    );
    assert!(!tree.matching.has_anchor());
}

#[test]
fn negative_complex_sibling_argument_is_linear_in_document_size() {
    use crate::budget::{BudgetKind, DocumentBudget, DocumentLimits, ResourceGovernor};
    let xml = format!("<r>{}</r>", "<i class='right'/>".repeat(96));
    let dom = crate::dom::xml::xml_dom(&xml).unwrap();
    let doc = crate::values::DocumentValues::detached(Default::default());
    let owner =
        DocumentBudget::new(DocumentLimits::default(), ResourceGovernor::default()).unwrap();
    let revision = owner.revision();
    doc.admit_revision(revision.clone(), None);
    let mut tree = TreeIndex::new(doc, true);
    let arg = parse_has_arg("~ .left ~ .right").unwrap();
    mark_has(&dom.document, &[arg], &mut vec![], &mut tree).unwrap();
    assert!(tree.has_marks.is_empty());
    assert!(
        revision.used(BudgetKind::TraversalVisits) < 10_000,
        "a negative result must not search the sibling suffix for every anchor: {} visits",
        revision.used(BudgetKind::TraversalVisits)
    );
}
