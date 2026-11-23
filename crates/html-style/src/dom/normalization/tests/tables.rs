//! Анонимные таблицы: границы рядов, контекст и атомарность сохранённого прохода.
use super::*;
use crate::dom::table_boxes::{InlineTables, wrap_anon_tables};
use crate::style::computed::{Display, Position};

fn cell(id: u64) -> Node {
    let mut n = node("td", vec![Node::Text(id.to_string().into())]);
    element_mut(&mut n).node_id = id;
    n
}
fn inline(children: Vec<Node>) -> Node {
    let mut n = node("span", children);
    element_mut(&mut n).inline = true;
    n
}
fn display(n: &Node) -> Option<Display> {
    element(n).style.used().reset_box_layout.display
}

#[test]
fn anonymous_table_groups_roles_with_internal_but_not_trailing_space() {
    let input = vec![
        Node::Text("before".into()),
        cell(7),
        Node::Text(" ".into()),
        cell(9),
        Node::Text(" ".into()),
        node("i", vec![]),
        cell(11),
        Node::Text(" ".into()),
    ];
    let output = wrap_anon_tables(&input);
    assert_eq!(output.len(), 6);
    let first = element(&output[1]);
    assert_eq!(display(&output[1]), Some(Display::Table));
    assert!(first.marks.anonymous_wrapper);
    assert_eq!(first.node_id, 7_u64.rotate_left(1) ^ 0x7ab1_e000);
    assert_eq!(first.children.len(), 3);
    assert!(matches!(&first.children[1], Node::Text(t) if t == " "));
    assert!(matches!(&output[2], Node::Text(t) if t == " "));
    assert_eq!(element(&output[4]).children.len(), 1);
    assert!(matches!(&output[5], Node::Text(t) if t == " "));
}

#[test]
fn table_roles_exclude_floats_and_positioned_but_keep_columns() {
    let mut floated = cell(1);
    element_mut(&mut floated)
        .style
        .cascaded_mut()
        .reset_box_layout
        .float = Some(1);
    let mut abs = cell(2);
    element_mut(&mut abs)
        .style
        .cascaded_mut()
        .reset_positioned
        .position = Some(Position::Absolute);
    let mut col = node("x", vec![]);
    let st = element_mut(&mut col).style.cascaded_mut();
    st.reset_box_layout.display = Some(Display::None);
    st.reset_context.col_role = Some(0);
    let output = wrap_anon_tables(&[floated, abs, col, cell(4)]).into_owned();
    assert_eq!(output.len(), 3);
    assert_eq!(element(&output[0]).tag, "td");
    assert_eq!(element(&output[1]).tag, "td");
    assert_eq!(element(&output[2]).children.len(), 2);
    let plain = vec![node("i", vec![])];
    assert!(matches!(
        wrap_anon_tables(&plain),
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn inline_table_context_changes_do_not_reuse_old_wrappers() {
    let raw: Children = vec![inline(vec![cell(1), cell(2)])].into();
    let mut pass = Pass::new(InlineTables);
    let old = publish(&mut pass, &raw, false, &Admission::default());
    assert_eq!(
        display(&element(&old[0]).children[0]),
        Some(Display::InlineTable)
    );
    let hit = publish(&mut pass, &raw, false, &Admission::default());
    assert_eq!(hit.identity(), old.identity());
    let mut changed = raw.clone();
    element_mut(&mut changed[0]).inline = false;
    let actual = publish(&mut pass, &changed, false, &Admission::default());
    assert_eq!(element(&actual[0]).children.len(), 2);
    assert_eq!(element(&element(&actual[0]).children[0]).tag, "td");
    let fresh: Children = apply(
        &InlineTables,
        changed.into_vec(),
        false,
        &Admission::default(),
    )
    .unwrap()
    .into();
    assert!(same(&actual, &fresh));
    assert_eq!(element(&old[0]).children.len(), 1);
}

#[test]
fn nested_inline_tables_prepare_abort_and_retry_match_full_conversion() {
    let raw: Children = vec![inline(vec![inline(vec![cell(1)]), cell(2)])].into();
    let mut pass = Pass::new(InlineTables);
    let old = publish(&mut pass, &raw, false, &Admission::default());
    let mut changed = raw.clone();
    element_mut(&mut changed[0]).children.push(cell(3));
    drop(
        pass.prepare(&changed, false, &Admission::default())
            .unwrap(),
    );
    assert_eq!(
        pass.current.as_ref().unwrap().output.identity(),
        old.identity()
    );
    let actual = publish(&mut pass, &changed, false, &Admission::default());
    let fresh: Children = apply(
        &InlineTables,
        changed.into_vec(),
        false,
        &Admission::default(),
    )
    .unwrap()
    .into();
    assert!(same(&actual, &fresh));
    assert_eq!(element(&element(&old[0]).children[1]).children.len(), 1);
    assert_eq!(element(&element(&actual[0]).children[1]).children.len(), 2);
    assert_eq!(
        display(&element(&element(&actual[0]).children[0]).children[0]),
        Some(Display::InlineTable)
    );
}
