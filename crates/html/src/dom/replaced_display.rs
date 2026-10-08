//! Replaced content keeps an inline box when given an internal table display.

use crate::computed::{Computed, Display, Position};

/// CSS Display 3 section 2.4 requires this used display before whitespace
/// removal and anonymous table fixup, rather than only when painting content.
pub(super) fn normalize(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
    let replaced = matches!(
        tag,
        "img" | "svg" | "canvas" | "video" | "iframe" | "embed" | "input"
    ) || tag == "object" && attrs.iter().any(|(name, _)| name == "data");
    let internal = matches!(
        style.display,
        Some(Display::TableCell | Display::TableRow | Display::TableRowGroup)
    ) || style.col_role.is_some() && style.display == Some(Display::None)
        || style.is_caption == Some(true);
    if replaced && internal {
        // CSS 2 section 9.7 blockifies out-of-flow internal table boxes;
        // they no longer have an internal computed display to normalize.
        let blockified = style.float.is_some_and(|value| value != 0)
            || matches!(style.position, Some(Position::Absolute | Position::Fixed));
        style.display = blockified.then_some(Display::Block);
        style.col_role = None;
        style.row_group_kind = None;
        style.is_caption = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_roles_are_cleared_before_anonymous_fixup() {
        for value in [
            "table-cell",
            "table-row",
            "table-row-group",
            "table-header-group",
            "table-footer-group",
            "table-column",
            "table-column-group",
            "table-caption",
        ] {
            let mut style = Computed::default();
            style.apply_one("display", value);
            normalize(&mut style, "img", &[]);
            assert_eq!(style.display, None, "{value}");
            assert_eq!(style.col_role, None, "{value}");
            assert_eq!(style.row_group_kind, None, "{value}");
            assert_eq!(style.is_caption, None, "{value}");
        }
    }

    #[test]
    fn non_replaced_table_cells_and_authored_none_keep_their_roles() {
        let mut style = Computed::default();
        style.apply_one("display", "table-cell");
        normalize(&mut style, "span", &[]);
        assert_eq!(style.display, Some(Display::TableCell));
        normalize(&mut style, "object", &[]);
        assert_eq!(style.display, Some(Display::TableCell));
        style.apply_one("display", "none");
        normalize(&mut style, "img", &[]);
        assert_eq!(style.display, Some(Display::None));
    }

    #[test]
    fn out_of_flow_replaced_boxes_keep_their_blockified_display() {
        for position in [None, Some(Position::Absolute), Some(Position::Fixed)] {
            let mut style = Computed::default();
            style.apply_one("display", "table-cell");
            style.position = position;
            style.float = position.is_none().then_some(-1);
            normalize(&mut style, "img", &[]);
            assert_eq!(style.display, Some(Display::Block));
        }
    }
}
