//! Reparented run-ins re-enter an unvisited target with its complete subtree.
use super::*;
use crate::dom::fixup_tree::flow_items::{RunIn, fold_run_ins};
use crate::style::computed::{Display, Position};

fn block(tag: &str, children: Vec<Node>, run: bool) -> Node {
    let mut n = node(tag, children);
    let style = element_mut(&mut n).style.cascaded_mut();
    style.reset_box_layout.display = Some(Display::Block);
    style.reset_context.run_in = run.then_some(true);
    n
}
fn fixture() -> Children {
    let mut wrapper = block(
        "absolute",
        vec![
            block("a", vec![], true),
            block("b", vec![], true),
            block("c", vec![], false),
        ],
        false,
    );
    element_mut(&mut wrapper)
        .style
        .cascaded_mut()
        .reset_positioned
        .position = Some(Position::Absolute);
    vec![
        block("outer", vec![wrapper], true),
        block("target", vec![], false),
    ]
    .into()
}
fn wrapper(output: &Children) -> &Element {
    let outer = &element(&output[0]).children[0];
    element(&element(outer).children[0])
}

#[test]
fn run_in_target_revisits_inserted_subtree_before_later_siblings() {
    let raw = fixture();
    let output: Children = fold_run_ins(&Admission::default(), raw.clone().into_vec(), None)
        .unwrap()
        .into();
    let retained = publish(&mut Pass::new(RunIn), &raw, false, &Admission::default());
    assert!(same(&output, &retained));
    assert_eq!(output.len(), 1);
    let wrapper = wrapper(&output);
    assert_eq!(wrapper.children.len(), 1);
    let c = element(&wrapper.children[0]);
    assert_eq!(c.tag, "c");
    assert_eq!(c.children.len(), 2);
    assert_eq!(element(&c.children[0]).tag, "a");
    assert_eq!(element(&c.children[1]).tag, "b");
    assert_eq!(raw.len(), 2);
    assert_eq!(element(&element(&raw[0]).children[0]).children.len(), 3);
}

#[test]
fn dirty_reparenting_matches_fresh_and_preserves_the_old_frame() {
    let raw = fixture();
    let mut pass = Pass::new(RunIn);
    let old = publish(&mut pass, &raw, false, &Admission::default());
    let again = publish(&mut pass, &raw, false, &Admission::default());
    assert_eq!(old.identity(), again.identity());
    let mut changed = raw.clone();
    let outer = element_mut(&mut changed[0]);
    let absolute = element_mut(&mut outer.children[0]);
    element_mut(&mut absolute.children[1])
        .style
        .cascaded_mut()
        .reset_context
        .run_in = None;
    let fresh: Children = fold_run_ins(&Admission::default(), changed.clone().into_vec(), None)
        .unwrap()
        .into();
    let actual = publish(&mut pass, &changed, false, &Admission::default());
    assert!(same(&fresh, &actual));
    assert_eq!(wrapper(&actual).children.len(), 2);
    assert_eq!(element(&wrapper(&actual).children[0]).tag, "b");
    assert_eq!(
        element(&element(&wrapper(&actual).children[0]).children[0]).tag,
        "a"
    );
    assert_eq!(wrapper(&old).children.len(), 1);
    assert_eq!(element(&wrapper(&old).children[0]).children.len(), 2);
    let abandoned = pass.prepare(&raw, false, &Admission::default()).unwrap();
    drop(abandoned);
    let after_abort = publish(&mut pass, &changed, false, &Admission::default());
    assert_eq!(actual.identity(), after_abort.identity());
}
