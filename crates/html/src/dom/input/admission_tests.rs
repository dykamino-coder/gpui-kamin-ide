//! Границы HTML/XML admission, terminal abort и владение partial state.

use super::{Admitted, Context, html, policy::Policy};
use crate::budget::{BudgetKind, DocumentBudget, DocumentLimits, ResourceGovernor};
use crate::dom::outcome::FailureReason;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

fn context(kind: BudgetKind, cap: u64) -> Context {
    Context::new(
        DocumentBudget::new(
            DocumentLimits::default().lower(kind, cap).unwrap(),
            ResourceGovernor::default(),
        )
        .unwrap()
        .revision(),
    )
}

fn facts(root: &Handle) -> (u64, String) {
    let (mut count, mut text, mut pending) = (0, String::new(), vec![root.clone()]);
    while let Some(node) = pending.pop() {
        count += 1;
        if let NodeData::Text { contents } = &node.data {
            text.push_str(&contents.borrow());
        }
        if let NodeData::Element {
            template_contents, ..
        } = &node.data
            && let Some(root) = template_contents.borrow().as_ref()
        {
            pending.push(root.clone());
        }
        pending.extend(node.children.borrow().iter().rev().cloned());
    }
    (count, text)
}

fn assert_released(context: Context) {
    let budget = context.budget().document().clone();
    drop(context);
    assert_eq!(budget.used(BudgetKind::LiveBytes), 0);
    assert_eq!(budget.governor().used(BudgetKind::LiveBytes), 0);
}

#[test]
fn html_implied_templates_slots_and_recovery_admit_actual_nodes() {
    for source in [
        "<p>KEEP</p><p>LOST</p>",
        "<table>KEEP<tr><td>LOST</table>",
        "<div><template>KEEP<slot>LOST</slot></template></div>",
        "<div><template shadowrootmode=open>KEEP<slot>LOST</slot></template></div>",
    ] {
        let admitted = Admitted::parse(source, false).unwrap();
        let (nodes, text) = facts(&admitted.dom.document);
        assert_eq!(text, "KEEPLOST");
        for cap in [nodes - 1, nodes, nodes + 1] {
            let context = context(BudgetKind::DomNodes, cap);
            let result = Admitted::with_context(source, false, context.clone());
            if cap < nodes {
                let failure = result.err().expect(source);
                assert!(matches!(failure.reason, FailureReason::Budget(_)));
            } else {
                assert_eq!(facts(&result.unwrap().dom.document), (nodes, text.clone()));
            }
            assert_released(context);
        }
    }
}

#[test]
fn text_and_attribute_boundaries_are_typed_and_release_all_storage() {
    for (source, kind, bytes) in [
        ("<p>KEEPLOST</p>", BudgetKind::TextBytes, 8),
        ("<p a='KEEP' b='LOST'></p>", BudgetKind::AttributeBytes, 10),
    ] {
        for cap in [bytes - 1, bytes, bytes + 1] {
            let context = context(kind, cap);
            let result = Admitted::with_context(source, false, context.clone());
            assert_eq!(result.is_ok(), cap >= bytes, "{kind:?}, cap={cap}");
            drop(result);
            assert_released(context);
        }
    }
}

#[test]
fn canonical_html_name_count_and_utf8_bytes_have_all_three_boundaries() {
    let source = "<div data-keep='KEEP'><custom-name data-lost='LOST'></custom-name></div>";
    let admitted = Admitted::parse(source, false).unwrap();
    for kind in [BudgetKind::UniqueNames, BudgetKind::NameBytes] {
        let needed = admitted.context().budget().used(kind);
        for cap in [needed - 1, needed, needed + 1] {
            let context = context(kind, cap);
            let result = Admitted::with_context(source, false, context.clone());
            assert_eq!(result.is_ok(), cap >= needed, "{kind:?}, cap={cap}");
            if let Err(failure) = &result {
                assert!(matches!(failure.reason, FailureReason::Budget(_)));
            }
            drop(result);
            assert_released(context);
        }
    }
}

#[test]
fn every_partial_storage_refusal_releases_parser_sink_and_name_set() {
    let source = "<p a='KEEP'>KEEP</p><table><b><tr><td>LOST</b></table>";
    for cap in (0..32768).step_by(97) {
        let context = context(BudgetKind::LiveBytes, cap);
        drop(Admitted::with_context(source, false, context.clone()));
        assert_released(context);
    }
}

#[test]
fn malformed_diagnostics_have_work_boundaries_and_release_owned_strings() {
    let source = "<p>KEEP</p><div a='&nope;' \"bad>LOST</div><!--unterminated";
    let admitted = Admitted::parse(source, false).unwrap();
    assert!(!admitted.dom.errors.borrow().is_empty());
    let needed = admitted
        .context()
        .budget()
        .used(BudgetKind::TraversalVisits);
    for cap in [needed - 1, needed, needed + 1] {
        let context = context(BudgetKind::TraversalVisits, cap);
        let result = Admitted::with_context(source, false, context.clone());
        assert_eq!(result.is_ok(), cap >= needed, "cap={cap}");
        drop(result);
        assert_released(context);
    }
}

#[test]
fn rejection_is_terminal_before_another_chunk_or_eof_repair() {
    let context = context(BudgetKind::DomNodes, 5);
    let policy = &*context.0;
    let dom = RcDom::try_new(policy).unwrap();
    let mut parser = html5ever::admission::parse_document(dom, policy, Default::default());
    parser.process_str("<p>KEEP</p>").unwrap_err();
    let work = context.budget().used(BudgetKind::TraversalVisits);
    let names = context.budget().used(BudgetKind::UniqueNames);
    assert!(parser.process_str("<LOST>unread</LOST>").is_err());
    assert_eq!(context.budget().used(BudgetKind::TraversalVisits), work);
    assert_eq!(context.budget().used(BudgetKind::UniqueNames), names);
    assert!(parser.finish().is_err());
    assert_eq!(context.budget().used(BudgetKind::TraversalVisits), work);
    assert_released(context);
}

#[test]
fn subparses_share_names_work_and_live_structural_counts() {
    let context = context(BudgetKind::DomNodes, 11);
    let first = Admitted::with_context("<p>KEEP</p>", false, context.clone()).unwrap();
    let names = context.budget().used(BudgetKind::UniqueNames);
    assert!(Admitted::with_context("<p>LOST</p>", false, context.clone()).is_err());
    assert_eq!(context.budget().used(BudgetKind::UniqueNames), names);
    drop(first);
    let next = Admitted::with_context("<p>LOST</p>", false, context.clone()).unwrap();
    assert_eq!(facts(&next.dom.document).1, "LOST");
    drop(next);
    assert_released(context);
}

#[test]
fn retained_dom_keeps_storage_alive_and_independent_documents_do_not_share_it() {
    let a = context(BudgetKind::DomNodes, 6);
    let b = context(BudgetKind::DomNodes, 6);
    let dom = html::parse("<p>KEEP</p>", &Policy::new(a.budget().clone())).unwrap();
    let other = Admitted::with_context("<p>LOST</p>", false, b.clone()).unwrap();
    assert!(a.budget().document().used(BudgetKind::LiveBytes) > 0);
    drop(dom);
    assert_eq!(a.budget().document().used(BudgetKind::LiveBytes), 0);
    assert!(b.budget().document().used(BudgetKind::LiveBytes) > 0);
    drop(other);
    assert_released(a);
    assert_released(b);
}

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
