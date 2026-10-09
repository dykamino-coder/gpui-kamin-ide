//! Orthogonal percentages retain a ratio-derived preferred containing-block size.
//! Content may expand the used height without changing that percentage basis.

use crate::layout::writing_mode::orthogonal_inline::{axis, edges};
use crate::style::computed::Computed;
use crate::style::computed::orthogonal::AxisSizes;
use crate::style::values::value::Len;

pub(crate) fn block_axis(parent: &Computed) -> AxisSizes {
    let mut block = axis(parent, true);
    if !matches!(parent.height, None | Some(Len::Auto)) {
        return block;
    }
    let Some(ratio) = parent.aspect_ratio.filter(|r| r.is_finite() && *r > 0.0) else {
        return block;
    };
    let inline = axis(parent, false);
    let Some(width) = inline.size else {
        return block;
    };
    let width = inline.clamp(width);
    let height = if parent.border_box == Some(true) {
        ((width + edges(parent, false)) / ratio - edges(parent, true)).max(0.0)
    } else {
        width / ratio
    };
    block.size = Some(block.clamp(height));
    block
}

#[cfg(test)]
mod tests {
    use crate::layout::writing_mode::orthogonal_inline::resolve;
    use super::*;

    #[test]
    fn ratio_parent_resolves_orthogonal_percentage_before_content_expands() {
        let parent = Computed {
            width: Some(Len::Px(100.0)),
            aspect_ratio: Some(1.0),
            ..Computed::default()
        };
        let mut child = Computed {
            height: Some(Len::Pct(1.0)),
            vertical: Some(true),
            ..Computed::default()
        };
        resolve(&mut child, &parent, (800.0, 600.0), false);
        assert_eq!(child.height, Some(Len::Px(100.0)));
        assert_eq!(child.orthogonal_inline.unwrap().fixed, Some(100.0));
    }

    #[test]
    fn preferred_ratio_basis_transfers_border_box_edges_and_constraints() {
        let mut parent = Computed {
            width: Some(Len::Px(140.0)),
            aspect_ratio: Some(2.0),
            border_box: Some(true),
            ..Computed::default()
        };
        parent.padding.left = Some(Len::Px(10.0));
        parent.padding.right = Some(Len::Px(10.0));
        parent.padding.top = Some(Len::Px(20.0));
        parent.padding.bottom = Some(Len::Px(20.0));
        assert_eq!(block_axis(&parent).size, Some(30.0));
        parent.min_height = Some(Len::Px(80.0));
        assert_eq!(block_axis(&parent).size, Some(40.0));
        parent.height = Some(Len::Px(120.0));
        assert_eq!(block_axis(&parent).size, Some(80.0));
    }
}
