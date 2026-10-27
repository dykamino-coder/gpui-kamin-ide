//! Границы и освобождение XML, включая сущности и синтаксические отказы.

use super::*;

#[test]
fn xml_nodes_names_work_and_payload_have_exact_boundaries() {
    let source = "<!DOCTYPE r [<!ENTITY keep 'KEEP'>]><r xmlns:p='urn:p' p:a='v'><template><slot>&keep;LOST</slot></template></r>";
    let admitted = Admitted::parse(source, true).unwrap();
    let (nodes, text) = facts(&admitted.dom.document);
    assert_eq!(text, "KEEPLOST");
    let budget = admitted.context().budget().clone();
    for (kind, needed) in [
        (BudgetKind::DomNodes, nodes),
        (BudgetKind::TextBytes, 8),
        (BudgetKind::AttributeBytes, 8),
        (
            BudgetKind::UniqueNames,
            budget.used(BudgetKind::UniqueNames),
        ),
        (BudgetKind::NameBytes, budget.used(BudgetKind::NameBytes)),
        (
            BudgetKind::TraversalVisits,
            budget.used(BudgetKind::TraversalVisits),
        ),
    ] {
        for cap in [needed - 1, needed, needed + 1] {
            let context = context(kind, cap);
            let result = Admitted::with_context(source, true, context.clone());
            assert_eq!(
                result.is_ok(),
                cap >= needed,
                "{kind:?}, cap={cap}: {:?}",
                result.as_ref().err()
            );
            if let Err(failure) = &result {
                assert!(matches!(failure.reason, FailureReason::Budget(_)));
            }
            drop(result);
            assert_released(context);
        }
    }
}

#[test]
fn xml_storage_refusals_release_the_core_and_partial_dom() {
    let source = "<!DOCTYPE r [<!ENTITY text 'KEEP'>]><r xmlns:p='urn:p' p:a='value'><!--KEEP--><p:c>&text;LOST</p:c></r>";
    for cap in (0..32768).step_by(97) {
        let context = context(BudgetKind::LiveBytes, cap);
        drop(Admitted::with_context(source, true, context.clone()));
        assert_released(context);
    }
    let context = context(BudgetKind::TraversalVisits, 10_000);
    let mut source = String::from("<!DOCTYPE r [<!ENTITY a0 'KEEP'>");
    for level in 1..16 {
        source.push_str(&format!(
            "<!ENTITY a{level} '&a{};&a{};'>",
            level - 1,
            level - 1
        ));
    }
    source.push_str("]><r>&a15;LOST</r>");
    let failure = Admitted::with_context(&source, true, context.clone())
        .err()
        .expect("acyclic entity expansion must stop");
    assert!(matches!(failure.reason, FailureReason::Budget(_)));
    assert_released(context);
}

#[test]
fn malformed_xml_is_typed_and_never_publishes_partial_state() {
    for source in [
        "<!DOCTYPE r [<!ENTITY cycle '&cycle;'>]><r>KEEP&cycle;LOST</r>",
        "<r>KEEP</wrong>LOST",
        "<r>KEEP&#invalid;LOST</r>",
        "<r>KEEP<p:LOST/></r>",
    ] {
        let context = context(BudgetKind::LiveBytes, 32768);
        let failure = Admitted::with_context(source, true, context.clone())
            .err()
            .unwrap();
        assert!(matches!(failure.reason, FailureReason::XmlSyntax { .. }));
        assert_released(context);
    }
}
