//! Full conversion and retained evaluation share hook order without cache records.
use super::*;
use crate::budget::{BudgetKind, DocumentBudget};

struct Hooks;

impl Transform for Hooks {
    type Context = i64;

    fn enter(&self, element: &mut Element, parent: &i64, _: &Admission) -> Result<i64, Error> {
        element.list_item = Some(*parent);
        Ok(parent + 1)
    }

    fn leave(&self, element: &mut Element, parent: &i64, _: &Admission) -> Result<(), Error> {
        let sum: i64 = element
            .children
            .iter()
            .filter_map(|n| match n {
                Node::Element(e) => e.list_item,
                _ => None,
            })
            .sum();
        element.list_item = Some(parent + sum);
        Ok(())
    }

    fn sequence(&self, nodes: &mut Children, _: &i64, _: &Admission) -> Result<(), Error> {
        nodes.reverse();
        Ok(())
    }
}

#[test]
fn owned_hooks_match_retained_enter_sequence_leave_at_every_depth() {
    let raw: Children = vec![
        node(
            "sum",
            vec![node("a", vec![]), node("sum", vec![node("b", vec![])])],
        ),
        Node::Text("between".into()),
        node("c", vec![]),
    ]
    .into();
    let full: Children = apply(&Hooks, raw.clone().into_vec(), 2, &Admission::default())
        .unwrap()
        .into();
    let retained = publish(&mut Pass::new(Hooks), &raw, 2, &Admission::default());
    assert!(same(&full, &retained));
    assert_eq!(element(&full[0]).tag, "c");
    assert_eq!(element(&full[2]).list_item, Some(12));
    assert_eq!(element(&raw[0]).list_item, None);
}

#[test]
fn owned_conversion_keeps_no_record_storage_after_return_or_failure() {
    let budget = DocumentBudget::new(Default::default(), Default::default()).unwrap();
    let raw: Children = (0..1024)
        .map(|_| node("b", vec![]))
        .collect::<Vec<_>>()
        .into();
    let output = apply(
        &Inherited::default(),
        raw.clone().into_vec(),
        3,
        &Admission::new(budget.revision()),
    )
    .unwrap();
    assert_eq!(output.len(), 1024);
    assert_eq!(budget.used(BudgetKind::LiveBytes), 0);
    let mut failed = raw.clone().into_vec();
    element_mut(&mut failed[512]).tag = "fail".into();
    assert!(matches!(
        apply(
            &Inherited::default(),
            failed,
            3,
            &Admission::new(budget.revision())
        ),
        Err(Error::Allocation)
    ));
    assert_eq!(budget.used(BudgetKind::LiveBytes), 0);
    assert_eq!(element(&raw[0]).list_item, None);
}

#[test]
fn deep_owned_conversion_uses_an_explicit_stack() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let mut raw = vec![Node::Text("bottom".into())];
            for _ in 0..6000 {
                raw = vec![node("b", raw)];
            }
            let output = apply(&Inherited::default(), raw, 2, &Admission::default()).unwrap();
            drop(output);
        })
        .unwrap()
        .join()
        .unwrap();
}
