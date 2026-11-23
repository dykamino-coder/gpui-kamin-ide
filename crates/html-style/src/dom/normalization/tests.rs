//! Differential checks for pass inputs, inherited context and ordered output.
use super::*;
use crate::dom::Node;
use std::cell::Cell;

mod contextual;
mod display;
mod hoist;
mod identity;
mod list_entry;
mod ordering;
mod owned;
mod siblings;
mod subgrid;
mod tables;
mod tails;
mod transaction;
mod transforms;

fn node(tag: &str, children: Vec<Node>) -> Node {
    Node::Element(Element::synthetic(
        tag,
        Default::default(),
        Default::default(),
        children,
    ))
}

fn element(node: &Node) -> &Element {
    let Node::Element(element) = node else {
        panic!("element expected")
    };
    element
}

fn element_mut(node: &mut Node) -> &mut Element {
    let Node::Element(element) = node else {
        panic!("element expected")
    };
    element
}

fn same(a: &Children, b: &Children) -> bool {
    let mut left = Element::synthetic("root", Default::default(), Default::default(), vec![]);
    let mut right = left.clone();
    left.children = a.clone();
    right.children = b.clone();
    crate::dom::same_tree::same_tree(&left, &right)
}

#[derive(Default)]
struct Inherited {
    visits: Rc<Cell<usize>>,
}

impl Transform for Inherited {
    type Context = i64;

    fn enter(&self, element: &mut Element, parent: &i64, _: &Admission) -> Result<i64, Error> {
        self.visits.set(self.visits.get() + 1);
        if element.tag == "fail" {
            return Err(Error::Allocation);
        }
        let context = if element.tag == "reset" { 7 } else { *parent };
        element.list_item = Some(context);
        Ok(context)
    }

    fn leave(&self, element: &mut Element, _: &i64, _: &Admission) -> Result<(), Error> {
        if element.tag == "sum" {
            element.list_item = Some(
                element
                    .children
                    .iter()
                    .map(|node| match node {
                        Node::Element(el) => el.list_item.unwrap(),
                        Node::Text(_) => 0,
                    })
                    .sum(),
            );
        }
        Ok(())
    }
}

fn publish<T: Transform>(
    pass: &mut Pass<T>,
    input: &Children,
    context: T::Context,
    admission: &Admission,
) -> Children {
    let prepared = pass.prepare(input, context, admission).unwrap();
    let output = prepared.output().clone();
    prepared.publish();
    output
}

#[test]
fn unchanged_input_reuses_root_without_entering_elements() {
    let input: Children = vec![node("sum", vec![node("reset", vec![node("b", vec![])])])].into();
    let meter = Inherited::default();
    let visits = meter.visits.clone();
    let mut pass = Pass::new(meter);
    let first = publish(&mut pass, &input, 2, &Admission::default());
    assert_eq!(visits.replace(0), 3);
    let second = publish(&mut pass, &input, 2, &Admission::default());
    assert_eq!(visits.get(), 0);
    assert_eq!(first.identity(), second.identity());
    assert_eq!(element(&second[0]).list_item, Some(7));
    assert_eq!(element(&input[0]).list_item, None);
}

#[test]
fn dirty_path_keeps_clean_descendant_lists_and_matches_fresh_pass() {
    let input: Children = vec![
        node("sum", vec![node("b", vec![Node::Text("old".into())])]),
        node("reset", vec![node("i", vec![node("u", vec![])])]),
    ]
    .into();
    let meter = Inherited::default();
    let visits = meter.visits.clone();
    let mut pass = Pass::new(meter);
    let old = publish(&mut pass, &input, 2, &Admission::default());
    let mut changed = input.clone();
    let branch = element_mut(&mut changed[0]);
    element_mut(&mut branch.children[0]).children = vec![Node::Text("new".into())].into();
    visits.set(0);
    let actual = publish(&mut pass, &changed, 2, &Admission::default());
    assert_eq!(visits.get(), 3); // Two dirty ancestors and the clean sibling's entry.
    assert_eq!(
        element(&actual[1]).children.identity(),
        element(&old[1]).children.identity()
    );
    let fresh = publish(
        &mut Pass::new(Inherited::default()),
        &changed,
        2,
        &Admission::default(),
    );
    assert!(same(&actual, &fresh));
    let Node::Text(text) = &element(&element(&old[0]).children[0]).children[0] else {
        panic!("text")
    };
    assert_eq!(text.as_str(), "old");
}

#[test]
fn changed_parent_context_revisits_inheritors_but_not_reset_descendants() {
    let input: Children = vec![
        node("sum", vec![node("b", vec![])]),
        node("reset", vec![node("i", vec![node("u", vec![])])]),
    ]
    .into();
    let meter = Inherited::default();
    let visits = meter.visits.clone();
    let mut pass = Pass::new(meter);
    let old = publish(&mut pass, &input, 2, &Admission::default());
    visits.set(0);
    let actual = publish(&mut pass, &input, 4, &Admission::default());
    assert_eq!(visits.get(), 3);
    assert_eq!(element(&actual[0]).list_item, Some(4));
    assert_eq!(
        element(&actual[1]).children.identity(),
        element(&old[1]).children.identity()
    );
    let fresh = publish(
        &mut Pass::new(Inherited::default()),
        &input,
        4,
        &Admission::default(),
    );
    assert!(same(&actual, &fresh));
}
