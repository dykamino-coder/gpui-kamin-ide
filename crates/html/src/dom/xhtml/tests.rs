//! XHTML admission проверяет границы до сканирования и удерживает буфер до drop.

use super::expand_xhtml_self_closing;
use crate::budget::{BudgetKind, DocumentBudget, DocumentLimits, RevisionBudget};
use crate::document::FailureReason;

fn budget(kind: BudgetKind, cap: u64) -> RevisionBudget {
    DocumentBudget::new(
        DocumentLimits::default().lower(kind, cap).unwrap(),
        Default::default(),
    )
    .unwrap()
    .revision()
}

#[test]
fn transformed_source_admits_all_three_work_boundaries_without_lost_text() {
    for source in [
        "<div/>KEEP<p/>LOST",
        "<style>p&gt;b{}<!--ignored--><![CDATA[KEEP]]></style>LOST",
        "<pre>\nKEEP</pre><textarea>\r\nLOST</textarea>",
    ] {
        let measured = budget(BudgetKind::TraversalVisits, 1 << 26);
        let expected = expand_xhtml_self_closing(source, &measured)
            .unwrap()
            .0
            .into_owned();
        assert!(expected.contains("KEEP") && expected.contains("LOST"));
        let work = measured.used(BudgetKind::TraversalVisits);
        for cap in [work - 1, work, work + 1] {
            let budget = budget(BudgetKind::TraversalVisits, cap);
            let result = expand_xhtml_self_closing(source, &budget);
            if cap < work {
                assert!(matches!(result.err().unwrap(), FailureReason::Budget(_)));
            } else {
                assert_eq!(result.unwrap().0, expected);
            }
            assert_eq!(budget.document().used(BudgetKind::LiveBytes), 0);
        }
    }
}

#[test]
fn rewritten_source_storage_is_admitted_before_growth_and_released_with_owner() {
    let source = "<div/>KEEP<p/>LOST";
    let bytes = (2 * source.len()) as u64;
    for cap in [bytes - 1, bytes, bytes + 1] {
        let budget = budget(BudgetKind::LiveBytes, cap);
        let result = expand_xhtml_self_closing(source, &budget);
        if cap < bytes {
            assert!(matches!(result.err().unwrap(), FailureReason::Budget(_)));
        } else {
            let owned = result.unwrap();
            assert_eq!(budget.document().used(BudgetKind::LiveBytes), bytes);
            assert!(owned.0.len() <= bytes as usize);
            drop(owned);
        }
        assert_eq!(budget.document().used(BudgetKind::LiveBytes), 0);
        assert_eq!(budget.document().governor().used(BudgetKind::LiveBytes), 0);
    }
}
