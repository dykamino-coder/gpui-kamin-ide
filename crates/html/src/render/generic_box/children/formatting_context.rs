//! Независимые контексты форматирования, общие для схлопывания полей и обхода line-clamp.

use crate::layout::multicol::spanner::multicol_container;
use crate::style::computed::{Computed, Display, Overflow, Position};

pub(super) fn independent(style: &Computed, tag: &str) -> bool {
    // CSS Display 3 §formatting-context and CSS Align 3 §align-block.
    let scroll = |value| matches!(value, Some(Overflow::Hidden | Overflow::Scroll));
    scroll(style.reset_box_layout.overflow_x)
        || scroll(style.reset_box_layout.overflow_y)
        || matches!(
            style.reset_positioned.position,
            Some(Position::Absolute | Position::Fixed)
        )
        || style.reset_box_layout.float.unwrap_or(0) != 0
        || style.reset_context.flow_root == Some(true)
        || style.reset_context.contain_layout == Some(true)
        || style.reset_context.contain_paint == Some(true)
        || style.reset_flex.align_content_block
        || multicol_container(style)
        || style.reset_fragmentation.column_span == Some(true)
        || (style.reset_box_layout.display == Some(Display::InlineBlock)
            && style.reset_context.inline_display != Some(true))
        || matches!(
            style.reset_box_layout.display,
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

    fn context(css: &str, vcx: crate::values::ValueCx<'_>) -> bool {
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
                reset_flex: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetFlex {
                        align_content_block: true,
                        ..Default::default()
                    }
                ),
                ..Computed::default()
            },
            "div"
        ));
    }
}
