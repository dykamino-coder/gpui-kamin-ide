//! Применение к GPUI: умолчание поперечной оси флекса в `vertical-rl`.
//! Прежний физический прижим к концу поперечной оси сохраняется только для
//! рядов, чья поперечная ось — физическая X (колонки по физике), без
//! собственного `align-items`; логическим вертикальным колонкам он не даётся.
use crate::style::computed::{Computed, Display, FlexDir};
pub(super) fn ends_on_x(c: &Computed, physical: Option<FlexDir>) -> bool {
    c.wm().block_rl()
        && (c.align_items.is_none() || super::layout::legacy_block(c))
        && matches!(c.display, Some(Display::Flex) | Some(Display::InlineFlex))
        && matches!(physical, Some(FlexDir::Col) | Some(FlexDir::ColReverse))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Align;
    #[test]
    fn logical_vertical_columns_do_not_get_a_physical_bottom_default() {
        for display in [Display::Flex, Display::InlineFlex] {
            let mut c = {
                let mut with_inherited = Computed {
                    display: Some(display),
                    ..Computed::default()
                };
                with_inherited.writing_mode = Some(
                    crate::style::computed::writing_mode::WritingModeKw::new(true, true, false),
                );
                with_inherited
            };
            for physical in [FlexDir::Row, FlexDir::RowReverse] {
                assert!(!ends_on_x(&c, Some(physical)));
            }
            for physical in [FlexDir::Col, FlexDir::ColReverse] {
                assert!(ends_on_x(&c, Some(physical)));
                c.align_items = Some(Align::Start);
                assert!(!ends_on_x(&c, Some(physical)));
                c.align_items = None;
            }
            c.writing_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
                true, false, false,
            ));
            assert!(!ends_on_x(&c, Some(FlexDir::Col)));
            c.writing_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
                true, true, false,
            ));
            c.display = Some(Display::Block);
            assert!(!ends_on_x(&c, Some(FlexDir::Col)));
        }
    }
}
