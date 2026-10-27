//! Проекция физического border box абзаца в блочную ось владельца.
use crate::layout::fragment::types::StackAxis;
use gpui::{Pixels, Point, Size};

pub(crate) fn block_offset(
    origin: Point<Pixels>,
    owner: Size<Pixels>,
    at: Point<Pixels>,
    paragraph: Size<Pixels>,
    axis: StackAxis,
) -> f32 {
    f32::from(match axis {
        StackAxis::Horizontal => at.y - origin.y,
        StackAxis::VerticalLr => at.x - origin.x,
        StackAxis::VerticalRl => origin.x + owner.width - at.x - paragraph.width,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, size};

    #[test]
    fn line_origins_follow_the_owners_block_start_in_every_writing_mode() {
        let origin = point(px(40.0), px(80.0));
        let owner = size(px(200.0), px(300.0));
        let at = point(px(100.0), px(130.0));
        let paragraph = size(px(30.0), px(70.0));
        for (axis, expected) in [
            (StackAxis::Horizontal, 50.0),
            (StackAxis::VerticalLr, 60.0),
            (StackAxis::VerticalRl, 110.0),
        ] {
            assert_eq!(block_offset(origin, owner, at, paragraph, axis), expected);
        }
        // Более широкий абзац продвигает block-start только при RL.
        let wide = size(px(50.0), px(70.0));
        assert_eq!(
            block_offset(origin, owner, at, wide, StackAxis::VerticalRl),
            90.0
        );
        assert_eq!(
            block_offset(origin, owner, at, wide, StackAxis::VerticalLr),
            60.0
        );
    }
}
