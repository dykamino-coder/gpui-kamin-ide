//! Every pass reads its own child output; later passes never enter raw records.
use super::*;

struct Scale;

impl Transform for Scale {
    type Context = ();

    fn enter(&self, element: &mut Element, _: &(), _: &Admission) -> Result<(), Error> {
        element.list_item = element.list_item.map(|value| value * 10);
        Ok(())
    }
}

struct Reverse;

impl Transform for Reverse {
    type Context = bool;

    fn enter(&self, _: &mut Element, reverse: &bool, _: &Admission) -> Result<bool, Error> {
        Ok(*reverse)
    }

    fn sequence(
        &self,
        nodes: Children,
        reverse: &bool,
        admission: &Admission,
    ) -> Result<Children, Error> {
        if !reverse || nodes.len() < 2 {
            return Ok(nodes);
        }
        let lease = admission.storage(Children::storage_bytes(nodes.len())?)?;
        admission.work(nodes.len())?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(nodes.len())
            .map_err(|_| Error::Allocation)?;
        output.extend(nodes.iter().rev().cloned());
        Ok(Children::from_reserved(output, lease))
    }
}

#[test]
fn later_pass_sees_postorder_output_and_stages_before_either_publication() {
    let input: Children = vec![node("sum", vec![node("reset", vec![]), node("b", vec![])])].into();
    let mut first = Pass::new(Inherited::default());
    let mut second = Pass::new(Scale);
    let admission = Admission::default();
    let staged_first = first.prepare(&input, 3, &admission).unwrap();
    assert_eq!(element(&staged_first.output()[0]).list_item, Some(10));
    let staged_second = second
        .prepare(staged_first.output(), (), &admission)
        .unwrap();
    let output = staged_second.output().clone();
    assert_eq!(element(&output[0]).list_item, Some(100));
    assert_eq!(
        element(&element(&output[0]).children[0]).list_item,
        Some(70)
    );
    staged_first.publish();
    staged_second.publish();

    let mut changed = input.clone();
    element_mut(&mut changed[0])
        .children
        .push(node("i", vec![]));
    let staged_first = first.prepare(&changed, 3, &admission).unwrap();
    let staged_second = second
        .prepare(staged_first.output(), (), &admission)
        .unwrap();
    assert_eq!(element(&staged_second.output()[0]).list_item, Some(130));
    drop(staged_second);
    drop(staged_first);
    assert_eq!(
        element(&first.current.as_ref().unwrap().output[0]).list_item,
        Some(10)
    );
    assert_eq!(
        second.current.as_ref().unwrap().output.identity(),
        output.identity()
    );

    let first_output = publish(&mut first, &changed, 3, &admission);
    let actual = publish(&mut second, &first_output, (), &admission);
    let fresh_first = publish(
        &mut Pass::new(Inherited::default()),
        &changed,
        3,
        &admission,
    );
    let fresh = publish(&mut Pass::new(Scale), &fresh_first, (), &admission);
    assert!(same(&actual, &fresh));
}

#[test]
fn sequence_reorder_keeps_per_input_records_when_siblings_move_or_disappear() {
    let input: Children = vec![
        node("a", vec![Node::Text("one".into())]),
        node("b", vec![Node::Text("two".into())]),
        node("c", vec![Node::Text("three".into())]),
    ]
    .into();
    let mut pass = Pass::new(Reverse);
    let old = publish(&mut pass, &input, true, &Admission::default());
    assert_eq!(element(&old[0]).tag, "c");
    let changed: Children = vec![input[1].clone(), input[0].clone()].into();
    let actual = publish(&mut pass, &changed, true, &Admission::default());
    assert_eq!(element(&actual[0]).tag, "a");
    assert_eq!(
        element(&actual[0]).children.identity(),
        element(&old[2]).children.identity()
    );
    let fresh = publish(
        &mut Pass::new(Reverse),
        &changed,
        true,
        &Admission::default(),
    );
    assert!(same(&actual, &fresh));
    let original_order = publish(&mut pass, &changed, false, &Admission::default());
    assert!(same(&original_order, &changed));
}
