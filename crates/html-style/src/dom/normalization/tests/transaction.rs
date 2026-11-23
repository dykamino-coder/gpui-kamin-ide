//! Prepared pass output is atomic, bounded and safe for deep trees.
use super::*;
use crate::budget::{BudgetKind, DocumentBudget, DocumentLimits};

#[test]
fn dropped_preparation_and_failed_walk_keep_published_output() {
    let input: Children = vec![node("sum", vec![node("b", vec![])])].into();
    let mut pass = Pass::new(Inherited::default());
    let old = publish(&mut pass, &input, 2, &Admission::default());
    let root = pass.current.clone().unwrap();
    let prepared = pass.prepare(&input, 4, &Admission::default()).unwrap();
    assert_eq!(element(&prepared.output()[0]).list_item, Some(4));
    drop(prepared);
    assert!(Rc::ptr_eq(pass.current.as_ref().unwrap(), &root));
    let mut changed = input.clone();
    element_mut(&mut changed[0])
        .children
        .push(node("fail", vec![]));
    assert!(matches!(
        pass.prepare(&changed, 4, &Admission::default()),
        Err(Error::Allocation)
    ));
    assert!(Rc::ptr_eq(pass.current.as_ref().unwrap(), &root));
    assert_eq!(element(&old[0]).list_item, Some(2));
    element_mut(&mut element_mut(&mut changed[0]).children[1]).tag = "i".into();
    let actual = publish(&mut pass, &changed, 4, &Admission::default());
    let fresh = publish(
        &mut Pass::new(Inherited::default()),
        &changed,
        4,
        &Admission::default(),
    );
    assert!(same(&actual, &fresh));
}

#[test]
fn storage_refusal_does_not_publish_or_retain_staged_storage() {
    let input: Children = vec![node("b", vec![])].into();
    let mut pass = Pass::new(Inherited::default());
    let old = publish(&mut pass, &input, 2, &Admission::default());
    let budget = DocumentBudget::new(
        DocumentLimits::default()
            .lower(BudgetKind::LiveBytes, 0)
            .unwrap(),
        Default::default(),
    )
    .unwrap();
    assert!(matches!(
        pass.prepare(&input, 4, &Admission::new(budget.revision())),
        Err(Error::Budget(_))
    ));
    assert_eq!(budget.used(BudgetKind::LiveBytes), 0);
    assert_eq!(
        pass.current.as_ref().unwrap().output.identity(),
        old.identity()
    );
}

#[test]
fn mid_walk_work_refusal_releases_prepared_records_and_output() {
    let input: Children = vec![node("sum", (0..20).map(|_| node("b", vec![])).collect())].into();
    let mut pass = Pass::new(Inherited::default());
    let old = publish(&mut pass, &input, 2, &Admission::default());
    let budget = DocumentBudget::new(
        DocumentLimits::default()
            .lower(BudgetKind::TraversalVisits, 10)
            .unwrap(),
        Default::default(),
    )
    .unwrap();
    assert!(matches!(
        pass.prepare(&input, 4, &Admission::new(budget.revision())),
        Err(Error::Budget(_))
    ));
    assert_eq!(budget.used(BudgetKind::LiveBytes), 0);
    assert_eq!(
        pass.current.as_ref().unwrap().output.identity(),
        old.identity()
    );
}

#[test]
fn replacements_have_flat_ledger_with_an_old_output_held_then_released() {
    let budget = DocumentBudget::new(Default::default(), Default::default()).unwrap();
    let mut pass = Pass::new(Inherited::default());
    let input: Children = vec![node("b", vec![Node::Text("text".into())])].into();
    let old = publish(&mut pass, &input, 2, &Admission::new(budget.revision()));
    let mut output = publish(&mut pass, &input, 3, &Admission::new(budget.revision()));
    let plateau = budget.used(BudgetKind::LiveBytes);
    for context in 4..10_004 {
        output = publish(
            &mut pass,
            &input,
            context,
            &Admission::new(budget.revision()),
        );
        assert_eq!(budget.used(BudgetKind::LiveBytes), plateau);
    }
    assert_eq!(element(&old[0]).list_item, Some(2));
    drop(old);
    assert!(budget.used(BudgetKind::LiveBytes) < plateau);
    drop(pass);
    assert!(budget.used(BudgetKind::LiveBytes) > 0); // The frame still holds the admitted output.
    assert_eq!(element(&output[0]).list_item, Some(10_003));
    drop(output);
    assert_eq!(budget.used(BudgetKind::LiveBytes), 0);
}

#[test]
fn deep_prepare_and_drop_use_an_explicit_stack() {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let mut input: Children = vec![Node::Text("bottom".into())].into();
            for _ in 0..6000 {
                let mut parent =
                    Element::synthetic("b", Default::default(), Default::default(), vec![]);
                parent.children = input;
                input = vec![Node::Element(parent)].into();
            }
            let mut pass = Pass::new(Inherited::default());
            let output = publish(&mut pass, &input, 2, &Admission::default());
            drop(input);
            drop(output);
            drop(pass);
        })
        .unwrap()
        .join()
        .unwrap();
}
