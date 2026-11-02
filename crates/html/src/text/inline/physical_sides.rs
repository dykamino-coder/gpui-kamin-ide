//! Физические украшения строчной коробки (отступы, рамки, цвета граней, радиусы углов) в
//! неповёрнутой плоскости набора абзаца: плоскость задаёт содержащий абзац, даже если строчный
//! элемент переопределяет `writing-mode`. Обёртка над `physical_projection` для `Sides` и
//! целого `Computed`.
use crate::style::computed::{Computed, Sides};

pub(super) fn project<T: Copy>(flow: &Computed, sides: [T; 4]) -> [T; 4] {
    super::physical_projection::project(
        flow.wm().turned(),
        flow.wm().line().line_turn()
            == crate::style::computed::writing_mode::Turn::CounterClockwise,
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
    style.reset_box_layout.padding = side_values(flow, style.reset_box_layout.padding);
    style.reset_border.border_width = side_values(flow, style.reset_border.border_width);
    let colors = style.reset_border.border_colors.clone();
    style.reset_border.border_colors = project(flow, [0, 1, 2, 3]).map(|i| colors[i].clone());
    style.reset_border.border_visible = project(flow, style.reset_border.border_visible);
    style.reset_border.border_side_styles = project(flow, style.reset_border.border_side_styles);
    // Corner order rotates by the same index permutation as edge order.
    let [tl, tr, br, bl] = project(
        flow,
        [
            style.reset_border.radius.tl,
            style.reset_border.radius.tr,
            style.reset_border.radius.br,
            style.reset_border.radius.bl,
        ],
    );
    style.reset_border.radius.tl = tl;
    style.reset_border.radius.tr = tr;
    style.reset_border.radius.br = br;
    style.reset_border.radius.bl = bl;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    fn flow(rotated: bool, ccw: bool) -> Computed {
        {
            let mut with_inherited = Computed {
                reset_text_box: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetTextBox {
                        turned_line: rotated,
                        ..Default::default()
                    },
                ),
                ..Computed::default()
            };
            with_inherited.inh.writing.flat_mode = Some(
                crate::style::computed::writing_mode::WritingModeKw::new(true, !ccw, ccw),
            );
            with_inherited
        }
    }

    #[test]
    fn empty_inline_box_projects_decorations_and_corners_together() {
        let mut style = Computed::default();
        style.reset_border.border_width = Sides {
            top: Some(Len::Px(2.0)),
            right: Some(Len::Px(3.0)),
            bottom: Some(Len::Px(5.0)),
            left: Some(Len::Px(7.0)),
        };
        style.reset_box_layout.padding = style.reset_border.border_width;
        style.reset_border.radius.tl = Some(Len::Px(11.0));
        style.reset_border.radius.tr = Some(Len::Px(13.0));
        style.reset_border.radius.br = Some(Len::Px(17.0));
        style.reset_border.radius.bl = Some(Len::Px(19.0));
        style.reset_border.border_visible = [Some(true), None, Some(false), Some(true)];
        let original = style.clone();
        project_box(&flow(true, false), &mut style);
        assert_eq!(style.reset_border.border_width.left, Some(Len::Px(2.0)));
        assert_eq!(style.reset_box_layout.padding.top, Some(Len::Px(3.0)));
        assert_eq!(style.reset_border.radius.tl, Some(Len::Px(13.0)));
        assert_eq!(
            style.reset_border.border_visible,
            [None, Some(false), Some(true), Some(true)]
        );
        project_box(&flow(true, true), &mut style);
        assert_eq!(
            style.reset_border.border_width,
            original.reset_border.border_width
        );
        assert_eq!(
            style.reset_box_layout.padding,
            original.reset_box_layout.padding
        );
        assert_eq!(style.reset_border.radius, original.reset_border.radius);
        assert_eq!(
            style.reset_border.border_visible,
            original.reset_border.border_visible
        );
    }
}
