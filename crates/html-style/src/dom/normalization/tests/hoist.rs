//! Перенос absolute в сетку: границы CB, порядок, наследование и abort.
use super::*;
use crate::dom::fixup_grid::hoist::HoistGridAbspos;
use crate::style::computed::{Display, Placement, Position};

fn identified(tag: &str, id: u64, children: Vec<Node>) -> Node {
    let mut n = node(tag, children);
    element_mut(&mut n).node_id = id;
    n
}
fn placed(id: u64) -> Node {
    let mut n = identified("abs", id, vec![]);
    let c = element_mut(&mut n).style.cascaded_mut();
    c.reset_positioned.position = Some(Position::Absolute);
    c.reset_grid.grid_col = Some((Placement::Line(1), Placement::Line(2)));
    n
}
fn grid(children: Vec<Node>) -> Node {
    let mut n = identified("grid", 1, children);
    let c = element_mut(&mut n).style.cascaded_mut();
    c.reset_box_layout.display = Some(Display::Grid);
    c.reset_positioned.position = Some(Position::Relative);
    n
}
fn fresh(input: Children) -> Children {
    apply(
        &HoistGridAbspos,
        input.into_vec(),
        (),
        &Admission::default(),
    )
    .unwrap()
    .into()
}

#[test]
fn grid_hoist_preserves_order_and_immediate_style_owner() {
    let raw: Children = vec![grid(vec![identified(
        "a",
        2,
        vec![
            placed(3),
            identified("b", 4, vec![placed(5)]),
            node("tail", vec![]),
        ],
    )])]
    .into();
    let mut pass = Pass::new(HoistGridAbspos);
    let output = publish(&mut pass, &raw, (), &Admission::default());
    let root = element(&output[0]);
    assert_eq!(root.children.len(), 3);
    assert_eq!(element(&root.children[1]).node_id, 3);
    assert_eq!(element(&root.children[2]).node_id, 5);
    assert_eq!(element(&root.children[1]).marks.style_parent, Some(2));
    assert_eq!(element(&root.children[2]).marks.style_parent, Some(4));
    let a = element(&root.children[0]);
    assert!(a.marks.lends_style);
    assert!(element(&a.children[0]).marks.lends_style);
    assert_eq!(a.children.len(), 2);
    assert!(same(&output, &fresh(raw.clone())));
    assert_eq!(element(&element(&raw[0]).children[0]).children.len(), 3);
    assert_eq!(
        publish(&mut pass, &raw, (), &Admission::default()).identity(),
        output.identity()
    );
}

#[test]
fn grid_hoist_stops_at_containing_blocks_subgrids_and_direct_grids() {
    let mut cb = identified("cb", 2, vec![placed(3)]);
    element_mut(&mut cb)
        .style
        .cascaded_mut()
        .reset_positioned
        .position = Some(Position::Relative);
    let mut subgrid = identified("sub", 4, vec![placed(5)]);
    element_mut(&mut subgrid)
        .style
        .cascaded_mut()
        .reset_grid
        .subgrid = true;
    let mut inner_grid = identified("inner", 6, vec![placed(7)]);
    element_mut(&mut inner_grid)
        .style
        .cascaded_mut()
        .reset_box_layout
        .display = Some(Display::Grid);
    let raw: Children = vec![grid(vec![
        cb,
        identified("wrap", 8, vec![subgrid]),
        inner_grid,
    ])]
    .into();
    let output = fresh(raw);
    let root = element(&output[0]);
    assert_eq!(root.children.len(), 3);
    assert_eq!(element(&root.children[0]).children.len(), 1);
    assert_eq!(
        element(&element(&root.children[1]).children[0])
            .children
            .len(),
        1
    );
    assert_eq!(element(&root.children[2]).children.len(), 1);
}

#[test]
fn grid_hoist_dirty_placement_abort_and_containing_block_change_match_fresh() {
    let raw: Children = vec![grid(vec![identified("owner", 2, vec![placed(3)])])].into();
    let mut pass = Pass::new(HoistGridAbspos);
    let old = publish(&mut pass, &raw, (), &Admission::default());
    let mut changed = raw.clone();
    let root = element_mut(&mut changed[0]);
    root.style.cascaded_mut().reset_positioned.position = Some(Position::Static);
    drop(pass.prepare(&changed, (), &Admission::default()).unwrap());
    assert_eq!(
        pass.current.as_ref().unwrap().output.identity(),
        old.identity()
    );
    let actual = publish(&mut pass, &changed, (), &Admission::default());
    assert!(same(&actual, &fresh(changed.clone())));
    assert_eq!(element(&actual[0]).children.len(), 1);
    assert_eq!(element(&old[0]).children.len(), 2);
    let owner = element_mut(&mut element_mut(&mut changed[0]).children[0]);
    element_mut(&mut owner.children[0])
        .style
        .cascaded_mut()
        .reset_grid
        .grid_col = None;
    element_mut(&mut changed[0])
        .style
        .cascaded_mut()
        .reset_positioned
        .position = Some(Position::Relative);
    let actual = publish(&mut pass, &changed, (), &Admission::default());
    assert!(same(&actual, &fresh(changed)));
    assert_eq!(element(&actual[0]).children.len(), 1);
}
