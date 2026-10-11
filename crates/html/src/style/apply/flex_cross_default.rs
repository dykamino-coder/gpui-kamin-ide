//! Preserve the legacy vertical row cross-end only on its physical X axis.
use crate::style::computed::{Computed, Display, FlexDir};
pub(super) fn ends_on_x(c: &Computed, physical: Option<FlexDir>) -> bool {
    c.vertical_rl == Some(true)
        && c.align_items.is_none()
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
            let mut c = Computed {
                vertical: Some(true),
                vertical_rl: Some(true),
                display: Some(display),
                ..Computed::default()
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
            c.vertical_rl = Some(false);
            assert!(!ends_on_x(&c, Some(FlexDir::Col)));
            c.vertical_rl = Some(true);
            c.display = Some(Display::Block);
            assert!(!ends_on_x(&c, Some(FlexDir::Col)));
        }
    }
}
