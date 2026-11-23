//! Real list-entry and tail passes keep their inherited and sibling boundaries.
use super::*;
use crate::dom::fixup_tree::quirks_height::QuirksHeight;
use crate::dom::float_tail::{FloatTail, TailContext};
use crate::style::{computed::Display, values::value::Len};

fn height(node: &Node) -> Option<Len> {
    element(node)
        .style
        .cascaded()
        .reset_box_layout
        .height
        .clone()
}
fn tail(node: &Node) -> bool {
    element(node).style.cascaded().reset_context.float_tail
}

#[test]
fn quirks_entry_marks_fill_before_children_and_breaks_at_tables() {
    let mut pct = node("div", vec![]);
    element_mut(&mut pct)
        .style
        .cascaded_mut()
        .reset_box_layout
        .height = Some(Len::Pct(0.5));
    let mut table = node("table", vec![pct.clone()]);
    let st = element_mut(&mut table).style.cascaded_mut();
    st.reset_box_layout.display = Some(Display::TableCell);
    st.reset_box_layout.height = Some(Len::Px(80.));
    let mut html = node(
        "html",
        vec![node("body", vec![node("div", vec![pct]), table])],
    );
    element_mut(&mut html)
        .style
        .cascaded_mut()
        .reset_box_layout
        .height = Some(Len::Px(200.));
    let raw: Children = vec![html].into();
    let mut pass = Pass::new(QuirksHeight);
    let old = publish(&mut pass, &raw, None, &Admission::default());
    let body = element(&element(&old[0]).children[0]);
    assert!(element(&old[0]).marks.quirks_fill && body.marks.quirks_fill);
    assert_eq!(
        height(&element(&body.children[0]).children[0]),
        Some(Len::Px(100.))
    );
    assert_eq!(
        height(&element(&body.children[1]).children[0]),
        Some(Len::Pct(0.5))
    );
    let mut dirty = raw.clone();
    let html = element_mut(&mut dirty[0]);
    html.style.cascaded_mut().reset_box_layout.height = Some(Len::Px(400.));
    element_mut(&mut html.children[0])
        .style
        .cascaded_mut()
        .reset_box_layout
        .float = Some(1);
    let prepared = pass.prepare(&dirty, None, &Admission::default()).unwrap();
    drop(prepared);
    let actual = publish(&mut pass, &dirty, None, &Admission::default());
    let fresh: Children = apply(&QuirksHeight, dirty.into_vec(), None, &Admission::default())
        .unwrap()
        .into();
    assert!(same(&actual, &fresh));
    assert!(!element(&element(&actual[0]).children[0]).marks.quirks_fill);
    assert!(element(&element(&old[0]).children[0]).marks.quirks_fill);
}

#[test]
fn float_tail_follows_only_the_last_nonblank_eligible_chain() {
    let raw: Children = vec![node(
        "div",
        vec![
            node("earlier", vec![node("deep", vec![])]),
            node("last", vec![node("tail", vec![])]),
            Node::Text(" ".into()),
        ],
    )]
    .into();
    let mut pass = Pass::new(FloatTail);
    let old = publish(
        &mut pass,
        &raw,
        TailContext::default(),
        &Admission::default(),
    );
    let kids = &element(&old[0]).children;
    assert!(tail(&old[0]) && tail(&kids[1]) && tail(&element(&kids[1]).children[0]));
    assert!(!tail(&kids[0]) && !tail(&element(&kids[0]).children[0]));
    let mut changed = raw.clone();
    element_mut(&mut changed[0])
        .children
        .push(Node::Text("visible".into()));
    let actual = publish(
        &mut pass,
        &changed,
        TailContext::default(),
        &Admission::default(),
    );
    assert!(!tail(&element(&actual[0]).children[1]));
    let fresh: Children = apply(
        &FloatTail,
        changed.into_vec(),
        TailContext::default(),
        &Admission::default(),
    )
    .unwrap()
    .into();
    assert!(same(&actual, &fresh));
    assert!(tail(&element(&old[0]).children[1]));
}

#[test]
fn float_tail_stops_at_items_floats_and_independent_contexts() {
    for display in [Display::Flex, Display::Grid, Display::InlineBlock] {
        let mut child = node("div", vec![node("deep", vec![])]);
        element_mut(&mut child)
            .style
            .cascaded_mut()
            .reset_box_layout
            .display = Some(display);
        let raw: Children = vec![node("div", vec![child])].into();
        let actual = publish(
            &mut Pass::new(FloatTail),
            &raw,
            TailContext::default(),
            &Admission::default(),
        );
        assert!(!tail(&element(&actual[0]).children[0]));
    }
    let mut root = node("div", vec![node("child", vec![])]);
    element_mut(&mut root)
        .style
        .cascaded_mut()
        .reset_box_layout
        .display = Some(Display::Flex);
    let output = publish(
        &mut Pass::new(FloatTail),
        &vec![root].into(),
        TailContext::default(),
        &Admission::default(),
    );
    // A flex root is not a block-like candidate in the original tail algorithm.
    assert!(!tail(&output[0]) && !tail(&element(&output[0]).children[0]));
}
