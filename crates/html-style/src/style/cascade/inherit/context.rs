//! Box stage, the parent box context: ruby, turned line and orthogonal flow facts, the
//! parent's grid/flex, the `line-clamp` limit, an ancestor's inline decorations, marker
//! suppression, text shadow ink.
//! These are used values of the box tree (`TreeFacts`): they never copy the shared computed style.

use super::*;
use crate::style::computed::BoxStyle;

pub(super) fn inherit_context(f: &ParentFacts<'_>, own: &Computed, c: &mut BoxStyle) {
    // What propagates through the element tree comes from its parent; orthogonal
    // flow limits and `line-clamp` come from the parent box.
    // `c.tree` holds the used values layout gave the box itself.
    let (layout, parent) = (&f.layout, &f.tree);
    inline_contents(c, parent);
    let own_clamp = c.reset_fragmentation.line_clamp;
    let tree = &mut c.tree;
    tree.text_shadow_ink = tree.text_shadow_ink.or(parent.text_shadow_ink);
    tree.ruby_unit |= parent.ruby_unit;
    tree.ruby_suppress_breaks |=
        own.reset_text_box.ruby_suppress_breaks || parent.ruby_suppress_breaks;
    tree.turned_line |= parent.turned_line;
    tree.ortho_limit = tree.ortho_limit.or(layout.ortho_limit);
    tree.orthogonal_scrollport = layout.orthogonal_scrollport;
    // An absolutely positioned box sizes its inline axis against its own
    // containing block (CSS 2.1 §10.3.7 / §10.6.4 shrink-to-fit), never with
    // the in-flow inline measure of the paragraph it was written in.
    tree.orthogonal_inline = if matches!(
        own.reset_positioned.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) {
        None
    } else {
        layout.orthogonal_inline
    };
    // A `-webkit-box` that is not a block container (no vertical orient)
    // is no line-clamp container and hands no clamp to its contents
    // (`line-clamp-029`).
    tree.line_clamp = own_clamp.or(layout.line_clamp);
    c.inline_decoration = c.inline_decoration.under(parent.inline_decoration);
    // css-ui-4 §6.1: `user-select` is not inherited; the used value of `auto`
    // is `none` under a box whose used value is `none`, selectable otherwise
    // (a parent's `all` is selectable too).
    use crate::style::computed::UserSelect as U;
    c.tree.no_select = match c.reset_context.user_select {
        Some(U::None) => Some(true),
        Some(U::Text | U::Contain | U::All) => Some(false),
        Some(U::Auto) | None => parent.no_select,
    };
}

/// Contents of the same line use its physical axes; an atomic box and an
/// out-of-flow element start their own layout (css-display-3 §2).
fn inline_contents(c: &mut BoxStyle, parent: &super::parent_facts::TreeFacts<'_>) {
    use crate::style::computed::{Display, InlineFormat, Position};
    use crate::style::values::value::Len;
    let inline = matches!(c.reset_box_layout.display, None | Some(Display::Contents))
        && !matches!(
            c.reset_positioned.position,
            Some(Position::Absolute | Position::Fixed)
        );
    // The box's own ruby unit (its `c.tree`, before the parent's is laid over it).
    if !(inline || c.tree.ruby_unit) {
        return;
    }
    if parent.ruby_unit {
        c.inh.text_flow.nowrap = Some(true);
    }
    let Some(format) = parent.inline_format else {
        return;
    };
    c.inline_format = Some(format);
    c.inh.writing.flat_mode = parent.flat_mode;
    c.inh.writing.writing_mode = None;
    c.tree.turned_line = parent.turned_line;
    c.reset_context.lines_reversed = parent.lines_reversed;
    let em = match c.inh.font.font_size {
        Some(Len::Px(px)) => px,
        _ => 16.0,
    };
    match format {
        InlineFormat::Turned => {}
        InlineFormat::Upright => {
            c.reset_context.upright_stack = true;
            c.inh.text_flow.break_word = Some(true);
            c.inh.font.line_height = Some(Len::Px(em));
        }
        InlineFormat::Combined => {
            c.inh.writing.combine_upright = Some(None);
            c.inh.text_flow.letter_spacing = Some(Len::Px(0.0));
            c.inh.font.line_height = Some(Len::Px(em));
        }
    }
}
