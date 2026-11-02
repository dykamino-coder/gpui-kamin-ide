//! Строчная коробка с письмом, отличным от родительского, — строчный flow root.
//!
//! css-writing-modes-4 §2.1: потоковая коробка с `display-inside: flow` и иным
//! `writing-mode` получает `flow-root`, то есть `display: inline` становится
//! `inline-block` (Blink `style_adjuster.cc`: kInline → kInlineBlock). Проход
//! по собранному дереву снимает `inline_display` у таких коробок; флоаты,
//! абсолютные и дети flex/grid блокифицируются и не трогаются.
use crate::dom::*;
use crate::style::computed::{Display, Position};

/// Resolved writing mode: (vertical, vertical-rl, sideways).
pub type Mode = (bool, bool, bool);

/// css-writing-modes-4 §2.1: «If a box has a different writing-mode value than
/// its parent box … if the box would otherwise become an in-flow box with a
/// display-inside of flow, its display-inside is instead flow-root» — an
/// in-flow `display: inline` box in another writing mode is an inline-block
/// (Blink `style_adjuster.cc`: kInline -> kInlineBlock when the writing mode
/// differs from the layout parent's). Without it a horizontal `display: inline`
/// item inside a vertical line was laid out as plain inline text
/// (grid-self-baseline-horiz-004-ref). Floats, absolutes and flex/grid items
/// are blockified instead, so they are left alone.
pub fn orthogonal_inlines_become_flow_roots(
    admission: &crate::budget::admission::Admission,
    nodes: &mut [Node],
    parent: Mode,
    parent_display: Option<Display>,
) -> Result<(), crate::budget::admission::Error> {
    crate::dom::traversal::preorder(
        admission,
        nodes,
        (parent, parent_display),
        |e, &(parent, parent_display)| {
            let blockifying = matches!(
                parent_display,
                Some(
                    Display::Flex
                        | Display::InlineFlex
                        | Display::Grid
                        | Display::InlineGrid
                        | Display::GridLanes
                )
            );

            let st = e.style.cascaded_mut();
            // `writing-mode` is inherited.
            let mode = match st.vertical_decl() {
                Some(vertical) => (vertical, st.wm().block_rl(), st.wm().sideways()),
                None => parent,
            };
            if mode != parent
                && !blockifying
                && st.reset_context.inline_display == Some(true)
                && st.reset_box_layout.display == Some(Display::InlineBlock)
                && st.reset_text_box.ruby_role.is_none()
                && st.reset_box_layout.float.is_none_or(|f| f == 0)
                && !matches!(
                    st.reset_positioned.position,
                    Some(Position::Absolute | Position::Fixed)
                )
            {
                st.reset_context.inline_display = None;
            }
            let display = st.reset_box_layout.display;
            Some((mode, display))
        },
    )
}
