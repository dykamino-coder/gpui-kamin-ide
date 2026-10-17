//! An inline box whose writing mode differs from its parent's becomes an inline flow root.
use crate::dom::*;
use crate::style::computed::{Display, Position};

/// Resolved writing mode: (vertical, vertical-rl, sideways).
pub(crate) type Mode = (bool, bool, bool);

/// css-writing-modes-4 §2.1: «If a box has a different writing-mode value than
/// its parent box … if the box would otherwise become an in-flow box with a
/// display-inside of flow, its display-inside is instead flow-root» — an
/// in-flow `display: inline` box in another writing mode is an inline-block
/// (Blink `style_adjuster.cc`: kInline -> kInlineBlock when the writing mode
/// differs from the layout parent's). Without it a horizontal `display: inline`
/// item inside a vertical line was laid out as plain inline text
/// (grid-self-baseline-horiz-004-ref). Floats, absolutes and flex/grid items
/// are blockified instead, so they are left alone.
pub(crate) fn orthogonal_inlines_become_flow_roots(
    nodes: &mut [Node],
    parent: Mode,
    parent_display: Option<Display>,
) {
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
    for node in nodes.iter_mut() {
        let Node::Element(e) = node else { continue };
        let st = &mut e.style;
        // `writing-mode` sets all three fields together; it is inherited.
        let mode = match st.vertical {
            Some(vertical) => (vertical, st.wm().block_rl(), st.wm().sideways()),
            None => parent,
        };
        if mode != parent
            && !blockifying
            && st.inline_display == Some(true)
            && st.display == Some(Display::InlineBlock)
            && st.ruby_role.is_none()
            && st.float.is_none_or(|f| f == 0)
            && !matches!(st.position, Some(Position::Absolute | Position::Fixed))
        {
            st.inline_display = None;
        }
        let display = st.display;
        orthogonal_inlines_become_flow_roots(&mut e.children, mode, display);
    }
}
