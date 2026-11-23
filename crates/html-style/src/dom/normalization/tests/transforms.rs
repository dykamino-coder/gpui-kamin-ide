//! Actual fixups reuse only their own raw inputs and complete inherited contexts.
use super::*;
use crate::dom::fixup_grid::lanes::LanesAsGrid;
use crate::dom::fixup_tree::{
    EmptyFilter,
    alignment::{AlignSelf, GridTableStretch},
    flow_items::FlexItemFloat,
};
use crate::dom::quirks_color::TableColor;
use crate::style::{
    computed::{Align, Display, Position},
    values::value::Color,
};

fn compare<T: Transform>(
    pass: &mut Pass<T>,
    transform: &T,
    raw: &Children,
    context: T::Context,
) -> Children {
    let full: Children = apply(
        transform,
        raw.clone().into_vec(),
        context.clone(),
        &Admission::default(),
    )
    .unwrap()
    .into();
    let retained = publish(pass, raw, context, &Admission::default());
    assert!(same(&full, &retained));
    retained
}

#[test]
fn align_self_tracks_parent_applicability_and_leaves_absolute_children() {
    let mut child = node("div", vec![]);
    let style = element_mut(&mut child).style.cascaded_mut();
    style.reset_flex.align_self = Some(Align::End);
    style.reset_flex.align_self_decl = Some(Some(Align::End));
    let mut absolute = child.clone();
    element_mut(&mut absolute)
        .style
        .cascaded_mut()
        .reset_positioned
        .position = Some(Position::Absolute);
    let raw: Children = vec![node("div", vec![child, absolute])].into();
    let mut pass = Pass::new(AlignSelf);
    let old = compare(&mut pass, &AlignSelf, &raw, None);
    assert_eq!(
        element(&element(&old[0]).children[0])
            .style
            .cascaded()
            .reset_flex
            .align_self,
        None
    );
    assert_eq!(
        element(&element(&old[0]).children[1])
            .style
            .cascaded()
            .reset_flex
            .align_self,
        Some(Align::End)
    );
    let mut changed = raw.clone();
    element_mut(&mut changed[0])
        .style
        .cascaded_mut()
        .reset_box_layout
        .display = Some(Display::Grid);
    let new = compare(&mut pass, &AlignSelf, &changed, None);
    assert_eq!(
        element(&element(&new[0]).children[0])
            .style
            .cascaded()
            .reset_flex
            .align_self,
        Some(Align::End)
    );
    assert_eq!(
        element(&element(&old[0]).children[0])
            .style
            .cascaded()
            .reset_flex
            .align_self,
        None
    );
}

#[test]
fn grid_stretch_and_flex_float_do_not_reuse_parent_edits_as_raw_children() {
    let mut table = node("table", vec![]);
    element_mut(&mut table)
        .style
        .cascaded_mut()
        .reset_box_layout
        .float = Some(1);
    let mut raw: Children = vec![node("div", vec![table])].into();
    element_mut(&mut raw[0])
        .style
        .cascaded_mut()
        .reset_box_layout
        .display = Some(Display::Grid);
    let mut stretch = Pass::new(GridTableStretch);
    let old = compare(&mut stretch, &GridTableStretch, &raw, ());
    assert_eq!(
        element(&element(&old[0]).children[0])
            .style
            .cascaded()
            .reset_flex
            .align_self,
        Some(Align::Stretch)
    );
    let mut floats = Pass::new(FlexItemFloat);
    let old_float = compare(&mut floats, &FlexItemFloat, &raw, ());
    assert_eq!(
        element(&element(&old_float[0]).children[0])
            .style
            .cascaded()
            .reset_box_layout
            .float,
        None
    );
    element_mut(&mut raw[0])
        .style
        .cascaded_mut()
        .reset_box_layout
        .display = Some(Display::Block);
    let new = compare(&mut stretch, &GridTableStretch, &raw, ());
    assert_eq!(
        element(&element(&new[0]).children[0])
            .style
            .cascaded()
            .reset_flex
            .align_self,
        None
    );
    let new_float = compare(&mut floats, &FlexItemFloat, &raw, ());
    assert_eq!(
        element(&element(&new_float[0]).children[0])
            .style
            .cascaded()
            .reset_box_layout
            .float,
        Some(1)
    );
}

#[test]
fn quirks_body_color_is_an_explicit_pass_wide_input() {
    let mut table = node("table", vec![]);
    element_mut(&mut table)
        .style
        .cascaded_mut()
        .reset_cascade
        .color_initial = true;
    let raw: Children = vec![node("div", vec![table])].into();
    let red = Color {
        r: 1.,
        g: 0.,
        b: 0.,
        a: 1.,
    };
    let blue = Color {
        r: 0.,
        g: 0.,
        b: 1.,
        a: 1.,
    };
    let mut pass = Pass::new(TableColor);
    let old = compare(&mut pass, &TableColor, &raw, Some(red));
    let new = compare(&mut pass, &TableColor, &raw, Some(blue));
    assert_eq!(
        element(&element(&old[0]).children[0])
            .style
            .cascaded()
            .inh
            .ui
            .color,
        Some(red)
    );
    assert_eq!(
        element(&element(&new[0]).children[0])
            .style
            .cascaded()
            .inh
            .ui
            .color,
        Some(blue)
    );
    let disabled = compare(&mut pass, &TableColor, &raw, None);
    assert!(same(&disabled, &raw));
}

#[test]
fn lanes_and_filter_recompute_from_changed_raw_input() {
    let mut lane = node("div", vec![]);
    element_mut(&mut lane)
        .style
        .cascaded_mut()
        .reset_box_layout
        .display = Some(Display::GridLanes);
    let mut raw: Children = vec![lane].into();
    let mut lanes = Pass::new(LanesAsGrid);
    let old = compare(&mut lanes, &LanesAsGrid, &raw, ());
    element_mut(&mut raw[0])
        .style
        .cascaded_mut()
        .reset_grid
        .lanes_inline = true;
    let new = compare(&mut lanes, &LanesAsGrid, &raw, ());
    assert_eq!(
        element(&old[0]).style.cascaded().reset_box_layout.display,
        Some(Display::Grid)
    );
    assert_eq!(
        element(&new[0]).style.cascaded().reset_box_layout.display,
        Some(Display::InlineGrid)
    );
    element_mut(&mut raw[0])
        .style
        .cascaded_mut()
        .reset_effects
        .filter_ref = Some("filter".into());
    element_mut(&mut raw[0]).children = vec![Node::Text(" ".into())].into();
    let mut filters = Pass::new(EmptyFilter);
    let empty = compare(&mut filters, &EmptyFilter, &raw, ());
    element_mut(&mut raw[0]).children = vec![Node::Text("content".into())].into();
    let filled = compare(&mut filters, &EmptyFilter, &raw, ());
    assert!(
        element(&empty[0])
            .style
            .cascaded()
            .reset_effects
            .filter_ref
            .is_some()
    );
    assert!(
        element(&filled[0])
            .style
            .cascaded()
            .reset_effects
            .filter_ref
            .is_none()
    );
}
