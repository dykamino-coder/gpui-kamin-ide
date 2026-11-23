//! Срезы подсетки пересчитываются сверху вниз, без повторного применения к готовому стилю.
use super::*;
use crate::dom::fixup_grid::subgrid_tracks::SubgridTracks;
use crate::style::computed::{Display, Placement, Position, Track, TrackSize};

fn grid(widths: &[f32], children: Vec<Node>) -> Node {
    let mut n = node("grid", children);
    let c = element_mut(&mut n).style.cascaded_mut();
    c.reset_box_layout.display = Some(Display::Grid);
    c.reset_grid.grid_tracks = Some(
        widths
            .iter()
            .map(|v| TrackSize::Single(Track::Px(*v)))
            .collect(),
    );
    n
}
fn subgrid(children: Vec<Node>) -> Node {
    let mut n = grid(&[], children);
    let g = &mut element_mut(&mut n).style.cascaded_mut().reset_grid;
    g.subgrid = true;
    g.subgrid_cols = true;
    g.grid_col = Some((Placement::Line(1), Placement::Span(2)));
    n
}
fn widths(n: &Node) -> Vec<f32> {
    element(n)
        .style
        .cascaded()
        .reset_grid
        .grid_tracks
        .as_ref()
        .unwrap()
        .iter()
        .map(|t| {
            let TrackSize::Single(Track::Px(v)) = t else {
                panic!("pixel track expected")
            };
            *v
        })
        .collect()
}
fn fresh(raw: Children) -> Children {
    apply(&SubgridTracks, raw.into_vec(), (), &Admission::default())
        .unwrap()
        .into()
}

#[test]
fn nested_subgrid_parent_edit_and_abort_match_full_top_down_conversion() {
    let raw: Children = vec![grid(&[30.0, 70.0], vec![subgrid(vec![subgrid(vec![])])])].into();
    let mut pass = Pass::new(SubgridTracks);
    let old = publish(&mut pass, &raw, (), &Admission::default());
    let child = &element(&old[0]).children[0];
    assert_eq!(widths(child), vec![30.0, 70.0]);
    assert_eq!(widths(&element(child).children[0]), vec![30.0, 70.0]);
    assert_eq!(
        publish(&mut pass, &raw, (), &Admission::default()).identity(),
        old.identity()
    );
    let mut changed = raw.clone();
    element_mut(&mut changed[0])
        .style
        .cascaded_mut()
        .reset_grid
        .grid_tracks = Some(vec![
        TrackSize::Single(Track::Px(10.0)),
        TrackSize::Single(Track::Px(90.0)),
    ]);
    drop(pass.prepare(&changed, (), &Admission::default()).unwrap());
    assert_eq!(
        pass.current.as_ref().unwrap().output.identity(),
        old.identity()
    );
    let actual = publish(&mut pass, &changed, (), &Admission::default());
    assert!(same(&actual, &fresh(changed)));
    let child = &element(&actual[0]).children[0];
    assert_eq!(widths(&element(child).children[0]), vec![10.0, 90.0]);
    assert_eq!(widths(&element(&old[0]).children[0]), vec![30.0, 70.0]);
}

#[test]
fn subgrid_cursor_counts_ordinary_items_and_inhibited_subgrid_clears_tracks() {
    let mut sub = subgrid(vec![]);
    element_mut(&mut sub)
        .style
        .cascaded_mut()
        .reset_grid
        .grid_col = Some((Placement::Span(2), Placement::Auto));
    let mut inhibited = subgrid(vec![]);
    element_mut(&mut inhibited)
        .style
        .cascaded_mut()
        .reset_positioned
        .position = Some(Position::Absolute);
    let raw: Children = vec![grid(
        &[10.0, 20.0, 30.0],
        vec![node("item", vec![]), sub, inhibited],
    )]
    .into();
    let output = fresh(raw);
    let children = &element(&output[0]).children;
    assert_eq!(widths(&children[1]), vec![20.0, 30.0]);
    assert!(
        element(&children[2])
            .style
            .cascaded()
            .reset_grid
            .grid_tracks
            .is_none()
    );
}
