//! Независимые контексты форматирования, общие для схлопывания полей и обхода line-clamp.

use crate::layout::multicol::spanner::multicol_container;
use crate::style::computed::{Computed, Display, Overflow, Position};

pub(super) fn independent(style: &Computed, tag: &str) -> bool {
    // CSS Display 3 §formatting-context and CSS Align 3 §align-block.
    let scroll = |value| matches!(value, Some(Overflow::Hidden | Overflow::Scroll));
    scroll(style.overflow_x)
        || scroll(style.overflow_y)
        || matches!(style.position, Some(Position::Absolute | Position::Fixed))
        || style.float.unwrap_or(0) != 0
        || style.flow_root == Some(true)
        || style.contain_layout == Some(true)
        || style.contain_paint == Some(true)
        || style.align_content_block
        || multicol_container(style)
        || style.column_span == Some(true)
        || (style.display == Some(Display::InlineBlock) && style.inline_display != Some(true))
        || matches!(
            style.display,
            Some(
                Display::Flex
                    | Display::InlineFlex
                    | Display::Grid
                    | Display::InlineGrid
                    | Display::GridLanes
                    | Display::Table
                    | Display::InlineTable
                    | Display::TableCell
            )
        )
        || tag == "fieldset"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::css::parse_decls;

    fn context(css: &str, vcx: crate::document::state::ValueCx<'_>) -> bool {
        let mut style = Computed::default();
        style.apply_decls(&parse_decls(css), vcx);
        independent(&style, "div")
    }

    #[test]
    fn ordinary_flow_and_clip_do_not_establish_contexts() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        for css in [
            "",
            "display: block",
            "display: list-item",
            "display: inline",
            "float: none",
            "overflow: visible",
            "overflow: clip",
            "position: relative",
        ] {
            assert!(!context(css, vcx), "{css}");
        }
    }

    #[test]
    fn independent_context_families_share_one_predicate() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        for css in [
            "display: flow-root",
            "display: inline-block",
            "display: flex",
            "display: inline-flex",
            "display: grid",
            "display: inline-grid",
            "display: table",
            "display: table-cell",
            "float: left",
            "float: right",
            "position: absolute",
            "position: fixed",
            "overflow-x: hidden",
            "overflow-y: scroll",
            "contain: layout",
            "contain: paint",
            "column-count: 2",
            "column-width: 30px",
            "column-span: all",
        ] {
            assert!(context(css, vcx), "{css}");
        }
        assert!(independent(&Computed::default(), "fieldset"));
        assert!(independent(
            &Computed {
                align_content_block: true,
                ..Computed::default()
            },
            "div"
        ));
    }
}
