//! A definite vertical containing block passes its content inline size to children.
use super::orthogonal_inline::{axis, edges};
use crate::computed::orthogonal::{AxisSizes, InlineConstraint};
use crate::computed::{Computed, Display};
use crate::value::Len;

pub(super) fn normal_block_flow(child: &Computed, parent: &Computed) -> bool {
    parent.vertical != Some(true)
        && matches!(parent.display, None | Some(Display::Block))
        && matches!(child.display, None | Some(Display::Block))
}

pub(super) fn containing_inline(parent: &Computed) -> Option<InlineConstraint> {
    // A parallel block fills its containing block's definite logical inline
    // size even while the parent's block size is being probed intrinsically.
    // Inline-blocks and table-cell flow roots also establish this containing
    // block; a wrapping fallback alone must not become a definite size.
    if parent.vertical == Some(true)
        && matches!(
            parent.display,
            None | Some(
                Display::Block | Display::InlineBlock | Display::ListItem | Display::TableCell
            )
        )
    {
        let inline = axis(parent, true);
        if let Some(size) = inline.size {
            let size = inline.clamp(size);
            return Some(InlineConstraint {
                available: size,
                fixed: Some(size),
                min: None,
                max: None,
            });
        }
    }
    parent.orthogonal_inline
}

pub(super) fn inherit(style: &mut Computed, mut parent: InlineConstraint) -> InlineConstraint {
    let used_parent = parent.fixed.map(|fixed| {
        AxisSizes {
            size: None,
            min: parent.min,
            max: parent.max,
        }
        .clamp(fixed)
    });
    if let Some(basis) = used_parent {
        // Percentage edges use the containing block's logical inline size.
        let resolve = |value: &mut Option<Len>| {
            if let Some(Len::Pct(k)) = *value {
                *value = Some(Len::Px(k * basis));
            }
        };
        for value in [
            &mut style.height,
            &mut style.min_height,
            &mut style.max_height,
            &mut style.margin.top,
            &mut style.margin.right,
            &mut style.margin.bottom,
            &mut style.margin.left,
            &mut style.padding.top,
            &mut style.padding.right,
            &mut style.padding.bottom,
            &mut style.padding.left,
        ] {
            resolve(value);
        }
    }
    let own = axis(style, true);
    if let Some(size) = own.size {
        return InlineConstraint {
            available: size,
            fixed: Some(size),
            min: own.min,
            max: own.max,
        };
    }
    if let Some(size) = used_parent {
        let margin = |value| match value {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let content =
            (size - edges(style, true) - margin(style.margin.top) - margin(style.margin.bottom))
                .max(0.0);
        parent = InlineConstraint {
            available: content,
            fixed: Some(content),
            min: own.min,
            max: own.max,
        };
    }
    // An indefinite parent's available space is not its intrinsic used size.
    parent
}

#[cfg(test)]
mod tests {
    use super::*;

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
        style.border_width.top = Some(Len::Px(20.0));
        style.border_width.bottom = Some(Len::Px(20.0));
        style.border_visible = [Some(true); 4];
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
        child.min_height = Some(Len::Px(70.0));
        let constraint = inherit(&mut child, cb);
        assert_eq!(constraint.fixed, Some(60.0));
        assert_eq!(constraint.used(10.0, 10.0), 70.0);
        cb.min = Some(200.0);
        assert_eq!(inherit(&mut bordered(), cb).fixed, Some(160.0));
    }

    #[test]
    fn authored_height_replaces_the_parent_and_border_box_edges_count_once() {
        let mut child = bordered();
        child.height = Some(Len::Px(80.0));
        assert_eq!(inherit(&mut child, parent(180.0)).fixed, Some(80.0));
        child.border_box = Some(true);
        assert_eq!(inherit(&mut child, parent(180.0)).fixed, Some(40.0));
    }

    #[test]
    fn percentage_edges_and_height_use_definite_content_inline_basis() {
        let mut child = Computed::default();
        child.margin.top = Some(Len::Pct(0.1));
        child.margin.bottom = Some(Len::Px(10.0));
        child.padding.top = Some(Len::Pct(0.1));
        assert_eq!(inherit(&mut child, parent(200.0)).fixed, Some(150.0));
        assert_eq!(child.margin.top, Some(Len::Px(20.0)));
        child.height = Some(Len::Pct(0.5));
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
        use super::super::{native_vertical, orthogonal_inline::resolve};
        let mut container = Computed {
            vertical: Some(true),
            display: Some(Display::InlineBlock),
            height: Some(Len::Px(160.0)),
            ortho_limit: Some(160.0),
            ..Computed::default()
        };
        let mut child = Computed {
            vertical: Some(true),
            display: Some(Display::Block),
            ..bordered()
        };
        resolve(&mut child, &container, (800.0, 600.0), false);
        let contribution = native_vertical::constraint(&child, 160.0).unwrap();
        assert_eq!(contribution.fixed, Some(120.0));
        assert_eq!(contribution.used(40.0, 140.0), 120.0);
        container.height = None;
        resolve(&mut child, &container, (800.0, 600.0), false);
        assert_eq!(child.orthogonal_inline, None);
        container.height = Some(Len::Px(160.0));
        container.display = Some(Display::Flex);
        resolve(&mut child, &container, (800.0, 600.0), false);
        assert_eq!(child.orthogonal_inline, None);
    }

    #[test]
    fn parallel_cell_children_fill_the_cells_authored_inline_size() {
        use super::super::{native_vertical, orthogonal_inline::resolve};
        let mut cell = Computed {
            vertical: Some(true),
            display: Some(Display::TableCell),
            height: Some(Len::Px(180.0)),
            ortho_limit: Some(180.0),
            ..Computed::default()
        };
        let mut child = Computed {
            vertical: Some(true),
            ..bordered()
        };
        resolve(&mut child, &cell, (800.0, 600.0), false);
        let constraint = native_vertical::constraint(&child, 180.0).unwrap();
        // A shorter line still occupies the containing cell's content inline
        // size, with both borders deducted once rather than shrinking its box.
        assert_eq!(constraint.used(60.0, 120.0), 140.0);
        cell.height = None;
        cell.ortho_col = true;
        resolve(&mut child, &cell, (800.0, 600.0), false);
        assert_eq!(child.orthogonal_inline, None);
    }
}
