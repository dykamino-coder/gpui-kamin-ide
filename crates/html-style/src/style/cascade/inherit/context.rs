//! Стадия коробки, контекст родителя-коробки: признаки руби, повёрнутой строки и ортогонального
//! потока, сетка/flex родителя, предел строк `line-clamp`, строчные украшения
//! предка, подавление маркера, чернила тени текста.
//! These are used values of the box tree (`TreeFacts`): they never copy the shared computed style.

use super::*;
use crate::style::computed::BoxStyle;

pub(super) fn inherit_context(parents: super::BoxParents<'_>, own: &Computed, c: &mut BoxStyle) {
    // Распространяемое по дереву элементов — от его родителя, ограничения
    // ортогонального потока и `line-clamp` — от коробки-родителя (`BoxParents`).
    // `c.tree` holds the used values layout gave the box itself.
    let super::BoxParents {
        layout,
        tree: parent,
    } = parents;
    c.inline_contents(parent, c.tree.ruby_unit);
    let own_clamp = c.reset_fragmentation.line_clamp;
    let tree = &mut c.tree;
    tree.text_shadow_ink = tree.text_shadow_ink.or(parent.tree.text_shadow_ink);
    tree.ruby_unit |= parent.tree.ruby_unit;
    tree.ruby_suppress_breaks |=
        own.reset_text_box.ruby_suppress_breaks || parent.tree.ruby_suppress_breaks;
    tree.turned_line |= parent.tree.turned_line;
    tree.ortho_limit = tree.ortho_limit.or(layout.tree.ortho_limit);
    tree.orthogonal_scrollport = layout.tree.orthogonal_scrollport;
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
        layout.tree.orthogonal_inline
    };
    // A `-webkit-box` that is not a block container (no vertical orient)
    // is no line-clamp container and hands no clamp to its contents
    // (`line-clamp-029`).
    tree.line_clamp = own_clamp.or(layout
        .tree
        .line_clamp
        .filter(|_| layout.clamp_block_container()));
    c.inline_decoration = c.inline_decoration.under(parent.inline_decoration);
    // css-ui-4 §6.1: `user-select` не наследуется; используемое значение
    // `auto` — `none` под коробкой с используемым `none`, иначе выделяемое
    // (`all` родителя тоже выделяемо).
    use crate::style::computed::UserSelect as U;
    c.tree.no_select = match c.reset_context.user_select {
        Some(U::None) => Some(true),
        Some(U::Text | U::Contain | U::All) => Some(false),
        Some(U::Auto) | None => parent.tree.no_select,
    };
}
