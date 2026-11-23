//! Parent formatting changes invalidate descendants without replaying prior fixups.
use super::*;
use crate::dom::fixup_tree::orthogonal_inline::OrthogonalInline;
use crate::dom::{grid_static_position::GridStaticPosition, subgrid_used::UsedSubgrid};
use crate::style::computed::{Display, Position, writing_mode::WritingModeKw};
use crate::style::values::value::Len;

fn check<T: Transform>(
    pass: &mut Pass<T>,
    transform: &T,
    raw: &Children,
    cx: T::Context,
) -> Children {
    let full: Children = apply(
        transform,
        raw.clone().into_vec(),
        cx.clone(),
        &Admission::default(),
    )
    .unwrap()
    .into();
    let retained = publish(pass, raw, cx, &Admission::default());
    assert!(same(&full, &retained));
    retained
}

fn nested_subgrid(nodes: &Children) -> &Element {
    element(&element(&element(&nodes[0]).children[0]).children[0])
}

#[test]
fn subgrid_tracks_parent_grid_through_contents_and_restores_raw_axes() {
    let mut child = node("div", vec![]);
    let style = element_mut(&mut child).style.cascaded_mut();
    style.reset_box_layout.display = Some(Display::Grid);
    style.reset_grid.subgrid = true;
    style.reset_grid.subgrid_cols = true;
    style.reset_grid.subgrid_rows = true;
    let mut contents = node("div", vec![child]);
    element_mut(&mut contents)
        .style
        .cascaded_mut()
        .reset_box_layout
        .display = Some(Display::Contents);
    let raw: Children = vec![node("div", vec![contents])].into();
    let mut pass = Pass::new(UsedSubgrid);
    let old = check(&mut pass, &UsedSubgrid, &raw, (None, false));
    assert!(!nested_subgrid(&old).style.cascaded().reset_grid.subgrid);
    let mut grid = raw.clone();
    element_mut(&mut grid[0])
        .style
        .cascaded_mut()
        .reset_box_layout
        .display = Some(Display::Grid);
    let new = check(&mut pass, &UsedSubgrid, &grid, (None, false));
    let axes = &nested_subgrid(&new).style.cascaded().reset_grid;
    assert!(axes.subgrid_cols && axes.subgrid_rows && axes.subgrid);
    assert!(!nested_subgrid(&old).style.cascaded().reset_grid.subgrid);
    let restored = check(&mut pass, &UsedSubgrid, &raw, (None, false));
    assert!(same(&old, &restored));
    assert!(nested_subgrid(&raw).style.cascaded().reset_grid.subgrid);
}

#[test]
fn changed_grid_padding_uses_pass_input_and_never_accumulates_old_margins() {
    let mut absolute = node("div", vec![]);
    let style = element_mut(&mut absolute).style.cascaded_mut();
    style.reset_positioned.position = Some(Position::Absolute);
    style.reset_box_layout.margin.top = Some(Len::Px(2.));
    let mut grid = node("div", vec![absolute]);
    let style = element_mut(&mut grid).style.cascaded_mut();
    style.reset_box_layout.display = Some(Display::Grid);
    style.reset_box_layout.padding.top = Some(Len::Px(10.));
    let raw: Children = vec![grid].into();
    let margin = |nodes: &Children| {
        element(&element(&nodes[0]).children[0])
            .style
            .cascaded()
            .reset_box_layout
            .margin
            .top
            .clone()
    };
    let mut pass = Pass::new(GridStaticPosition);
    let old = check(&mut pass, &GridStaticPosition, &raw, false);
    assert_eq!(margin(&old), Some(Len::Px(12.)));
    let again = check(&mut pass, &GridStaticPosition, &raw, false);
    assert_eq!(old.identity(), again.identity());
    let mut changed = raw.clone();
    element_mut(&mut changed[0])
        .style
        .cascaded_mut()
        .reset_box_layout
        .padding
        .top = Some(Len::Px(20.));
    let new = check(&mut pass, &GridStaticPosition, &changed, false);
    assert_eq!(margin(&new), Some(Len::Px(22.)));
    assert_eq!(margin(&old), Some(Len::Px(12.)));
    assert_eq!(margin(&raw), Some(Len::Px(2.)));
}

#[test]
fn orthogonal_inline_reconsiders_parent_mode_and_blockification() {
    let mut inline = node("span", vec![]);
    let style = element_mut(&mut inline).style.cascaded_mut();
    style.inh.writing.writing_mode = Some(WritingModeKw::VerticalRl);
    style.reset_context.inline_display = Some(true);
    style.reset_box_layout.display = Some(Display::InlineBlock);
    let raw: Children = vec![inline].into();
    let mut pass = Pass::new(OrthogonalInline);
    let horizontal = ((false, false, false), None);
    let old = check(&mut pass, &OrthogonalInline, &raw, horizontal);
    assert_eq!(
        element(&old[0])
            .style
            .cascaded()
            .reset_context
            .inline_display,
        None
    );
    let new = check(
        &mut pass,
        &OrthogonalInline,
        &raw,
        ((true, true, false), None),
    );
    assert_eq!(
        element(&new[0])
            .style
            .cascaded()
            .reset_context
            .inline_display,
        Some(true)
    );
    let grid = check(
        &mut pass,
        &OrthogonalInline,
        &raw,
        (horizontal.0, Some(Display::Grid)),
    );
    assert_eq!(
        element(&grid[0])
            .style
            .cascaded()
            .reset_context
            .inline_display,
        Some(true)
    );
    assert_eq!(
        element(&old[0])
            .style
            .cascaded()
            .reset_context
            .inline_display,
        None
    );
}
