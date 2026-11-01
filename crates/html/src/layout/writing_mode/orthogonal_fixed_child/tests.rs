//! Проверки поведения доменного модуля.
use super::*;
use crate::style::computed::BoxStyle;

fn parent(size: f32) -> InlineConstraint {
    InlineConstraint {
        available: size,
        fixed: Some(size),
        min: None,
        max: None,
    }
}

fn bordered() -> Computed {
    let mut style = Computed::default();
    style.reset_border.border_width.top = Some(Len::Px(20.0));
    style.reset_border.border_width.bottom = Some(Len::Px(20.0));
    style.reset_border.border_visible = [Some(true); 4];
    style
}

#[test]
fn each_nested_block_deducts_its_own_inline_edges() {
    let first = inherit(&mut bordered(), parent(180.0));
    let second = inherit(&mut bordered(), first);
    assert_eq!(first.fixed, Some(140.0));
    assert_eq!(second.fixed, Some(100.0));
    assert_eq!(second.available, 100.0);
}

#[test]
fn containing_block_limits_apply_before_child_edges_and_own_limits() {
    let mut cb = parent(180.0);
    cb.max = Some(100.0);
    let mut child = bordered();
    child.reset_box_layout.min_height = Some(Len::Px(70.0));
    let constraint = inherit(&mut child, cb);
    assert_eq!(constraint.fixed, Some(60.0));
    assert_eq!(constraint.used(10.0, 10.0), 70.0);
    cb.min = Some(200.0);
    assert_eq!(inherit(&mut bordered(), cb).fixed, Some(160.0));
}

#[test]
fn authored_height_replaces_the_parent_and_border_box_edges_count_once() {
    let mut child = bordered();
    child.reset_box_layout.height = Some(Len::Px(80.0));
    assert_eq!(inherit(&mut child, parent(180.0)).fixed, Some(80.0));
    child.reset_border.border_box = Some(true);
    assert_eq!(inherit(&mut child, parent(180.0)).fixed, Some(40.0));
}

#[test]
fn percentage_edges_and_height_use_definite_content_inline_basis() {
    let mut child = Computed::default();
    child.reset_box_layout.margin.top = Some(Len::Pct(0.1));
    child.reset_box_layout.margin.bottom = Some(Len::Px(10.0));
    child.reset_box_layout.padding.top = Some(Len::Pct(0.1));
    assert_eq!(inherit(&mut child, parent(200.0)).fixed, Some(150.0));
    assert_eq!(child.reset_box_layout.margin.top, Some(Len::Px(20.0)));
    child.reset_box_layout.height = Some(Len::Pct(0.5));
    assert_eq!(inherit(&mut child, parent(200.0)).fixed, Some(100.0));
}

#[test]
fn indefinite_parent_is_not_promoted_to_a_definite_containing_block() {
    let mut cb = parent(180.0);
    cb.fixed = None;
    assert_eq!(inherit(&mut bordered(), cb), cb);
}

#[test]
fn parallel_block_retains_inline_block_height_during_intrinsic_measurement() {
    use crate::layout::writing_mode::native_vertical;
    use crate::layout::writing_mode::orthogonal_inline::resolve;
    let mut container = {
        let mut with_inherited = Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    display: Some(Display::InlineBlock),
                    height: Some(Len::Px(160.0)),
                    ..Default::default()
                },
            ),
            reset_context: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetContext {
                    ortho_limit: Some(160.0),
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        with_inherited.inh.writing.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    let mut child = {
        let mut with_inherited = {
            let style_base = bordered();
            Computed {
                reset_box_layout: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetBoxLayout {
                        display: Some(Display::Block),
                        ..(*style_base.reset_box_layout).clone()
                    },
                ),
                ..style_base
            }
        };
        with_inherited.inh.writing.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    resolve(&mut child, &container, (800.0, 600.0), false);
    let contribution = native_vertical::constraint(&BoxStyle::new(child.clone()), 160.0).unwrap();
    assert_eq!(contribution.fixed, Some(120.0));
    assert_eq!(contribution.used(40.0, 140.0), 120.0);
    container.reset_box_layout.height = None;
    resolve(&mut child, &container, (800.0, 600.0), false);
    assert_eq!(child.reset_context.orthogonal_inline, None);
    container.reset_box_layout.height = Some(Len::Px(160.0));
    container.reset_box_layout.display = Some(Display::Flex);
    resolve(&mut child, &container, (800.0, 600.0), false);
    assert_eq!(child.reset_context.orthogonal_inline, None);
}

#[test]
fn parallel_cell_children_fill_the_cells_authored_inline_size() {
    use crate::layout::writing_mode::native_vertical;
    use crate::layout::writing_mode::orthogonal_inline::resolve;
    let mut cell = {
        let mut with_inherited = Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    display: Some(Display::TableCell),
                    height: Some(Len::Px(180.0)),
                    ..Default::default()
                },
            ),
            reset_context: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetContext {
                    ortho_limit: Some(180.0),
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        with_inherited.inh.writing.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    let mut child = {
        let mut with_inherited = Computed { ..bordered() };
        with_inherited.inh.writing.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    resolve(&mut child, &cell, (800.0, 600.0), false);
    let constraint = native_vertical::constraint(&BoxStyle::new(child.clone()), 180.0).unwrap();
    // A shorter line still occupies the containing cell's content inline
    // size, with both borders deducted once rather than shrinking its box.
    assert_eq!(constraint.used(60.0, 120.0), 140.0);
    cell.reset_box_layout.height = None;
    cell.reset_context.ortho_col = true;
    resolve(&mut child, &cell, (800.0, 600.0), false);
    assert_eq!(child.reset_context.orthogonal_inline, None);
}
