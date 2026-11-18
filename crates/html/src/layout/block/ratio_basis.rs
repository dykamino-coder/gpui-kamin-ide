//! База процентов ортогонального ребёнка у родителя с `aspect-ratio` и высотой `auto`:
//! блочный размер содержащего блока выводится из ширины через пропорцию (с учётом
//! `box-sizing` и ограничений `min/max-height`) и остаётся базой долей, даже если
//! содержимое потом растянет использованную высоту. Служит `orthogonal_inline::resolve`.

use crate::layout::writing_mode::orthogonal_inline::{axis, edges};
use crate::style::computed::Computed;
use crate::style::computed::orthogonal::AxisSizes;
use crate::style::values::value::Len;

pub(crate) fn block_axis(parent: &Computed) -> AxisSizes {
    let mut block = axis(parent, true);
    if !matches!(parent.reset_box_layout.height, None | Some(Len::Auto)) {
        return block;
    }
    let Some(ratio) = parent
        .reset_shapes
        .aspect_ratio
        .filter(|r| r.is_finite() && *r > 0.0)
    else {
        return block;
    };
    let inline = axis(parent, false);
    let Some(width) = inline.size else {
        return block;
    };
    let width = inline.clamp(width);
    let height = if parent.reset_border.border_box == Some(true) {
        ((width + edges(parent, false)) / ratio - edges(parent, true)).max(0.0)
    } else {
        width / ratio
    };
    block.size = Some(block.clamp(height));
    block
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::writing_mode::orthogonal_inline::resolve;

    #[test]
    fn ratio_parent_resolves_orthogonal_percentage_before_content_expands() {
        let parent = Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    width: Some(Len::Px(100.0)),
                    ..Default::default()
                },
            ),
            reset_shapes: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetShapes {
                    aspect_ratio: Some(1.0),
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        let mut child = {
            let mut with_inherited = Computed {
                reset_box_layout: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetBoxLayout {
                        height: Some(Len::Pct(1.0)),
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
        resolve(&mut child, &parent, (800.0, 600.0), false);
        assert_eq!(child.reset_box_layout.height, Some(Len::Px(100.0)));
        assert_eq!(
            child.reset_context.orthogonal_inline.unwrap().fixed,
            Some(100.0)
        );
    }

    #[test]
    fn preferred_ratio_basis_transfers_border_box_edges_and_constraints() {
        let mut parent = Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    width: Some(Len::Px(140.0)),
                    ..Default::default()
                },
            ),
            reset_shapes: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetShapes {
                    aspect_ratio: Some(2.0),
                    ..Default::default()
                },
            ),
            reset_border: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBorder {
                    border_box: Some(true),
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        parent.reset_box_layout.padding.left = Some(Len::Px(10.0));
        parent.reset_box_layout.padding.right = Some(Len::Px(10.0));
        parent.reset_box_layout.padding.top = Some(Len::Px(20.0));
        parent.reset_box_layout.padding.bottom = Some(Len::Px(20.0));
        assert_eq!(block_axis(&parent).size, Some(30.0));
        parent.reset_box_layout.min_height = Some(Len::Px(80.0));
        assert_eq!(block_axis(&parent).size, Some(40.0));
        parent.reset_box_layout.height = Some(Len::Px(120.0));
        assert_eq!(block_axis(&parent).size, Some(80.0));
    }
}
