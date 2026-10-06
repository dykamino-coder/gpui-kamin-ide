//! Convert physical inline decorations to the paragraph's unrotated shaping plane.
//! The containing paragraph fixes this plane even if an inline overrides writing-mode.
use crate::computed::{Computed, Sides};

pub(super) fn project<T: Copy>(flow: &Computed, sides: [T; 4]) -> [T; 4] {
    super::physical_projection::project(
        flow.rotated_line == Some(true),
        flow.sideways == Some(true) && flow.vertical_rl != Some(true),
        sides,
    )
}

pub(crate) fn side_values(flow: &Computed, sides: Sides) -> Sides {
    let [top, right, bottom, left] =
        project(flow, [sides.top, sides.right, sides.bottom, sides.left]);
    Sides {
        top,
        right,
        bottom,
        left,
    }
}

pub(super) fn project_box(flow: &Computed, style: &mut Computed) {
    style.padding = side_values(flow, style.padding);
    style.border_width = side_values(flow, style.border_width);
    style.border_colors = project(flow, style.border_colors);
    style.border_visible = project(flow, style.border_visible);
    style.border_side_styles = project(flow, style.border_side_styles);
    style.border_side_current = project(flow, style.border_side_current);
    // Corner order rotates by the same index permutation as edge order.
    let [tl, tr, br, bl] = project(
        flow,
        [
            style.radius.tl,
            style.radius.tr,
            style.radius.br,
            style.radius.bl,
        ],
    );
    style.radius.tl = tl;
    style.radius.tr = tr;
    style.radius.br = br;
    style.radius.bl = bl;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Len;

    fn flow(rotated: bool, ccw: bool) -> Computed {
        Computed {
            rotated_line: Some(rotated),
            sideways: Some(ccw),
            vertical_rl: Some(!ccw),
            ..Computed::default()
        }
    }

    #[test]
    fn empty_inline_box_projects_decorations_and_corners_together() {
        let mut style = Computed::default();
        style.border_width = Sides {
            top: Some(Len::Px(2.0)),
            right: Some(Len::Px(3.0)),
            bottom: Some(Len::Px(5.0)),
            left: Some(Len::Px(7.0)),
        };
        style.padding = style.border_width;
        style.radius.tl = Some(Len::Px(11.0));
        style.radius.tr = Some(Len::Px(13.0));
        style.radius.br = Some(Len::Px(17.0));
        style.radius.bl = Some(Len::Px(19.0));
        style.border_visible = [Some(true), None, Some(false), Some(true)];
        let original = style.clone();
        project_box(&flow(true, false), &mut style);
        assert_eq!(style.border_width.left, Some(Len::Px(2.0)));
        assert_eq!(style.padding.top, Some(Len::Px(3.0)));
        assert_eq!(style.radius.tl, Some(Len::Px(13.0)));
        assert_eq!(
            style.border_visible,
            [None, Some(false), Some(true), Some(true)]
        );
        project_box(&flow(true, true), &mut style);
        assert_eq!(style.border_width, original.border_width);
        assert_eq!(style.padding, original.padding);
        assert_eq!(style.radius, original.radius);
        assert_eq!(style.border_visible, original.border_visible);
    }
}
