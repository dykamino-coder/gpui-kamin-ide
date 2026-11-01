//! Стадия коробки, контекст родителя-коробки: признаки руби, повёрнутой строки и ортогонального
//! потока, сетка/flex родителя, предел строк `line-clamp`, строчные украшения
//! предка, подавление маркера, чернила тени текста.

use super::*;
use crate::style::computed::BoxStyle;

/// TRANSITIONAL(cascade-7b-R10): Запись box context ребёнка копирует общий Computed через BoxStyle::DerefMut; удалить после переноса этих полей в BoxStyle и перевода paint/layout-made readers.
pub(super) fn inherit_context(parents: super::BoxParents<'_>, own: &Computed, c: &mut BoxStyle) {
    // Распространяемое по дереву элементов — от его родителя, ограничения
    // ортогонального потока и `line-clamp` — от коробки-родителя (`BoxParents`).
    let super::BoxParents {
        layout,
        tree: parent,
    } = parents;
    c.inline_contents(parent, own.reset_text_box.ruby_unit);
    set!(
        c.reset_text_box.text_shadow_ink,
        own.reset_text_box
            .text_shadow_ink
            .or(parent.reset_text_box.text_shadow_ink)
    );
    set!(
        c.reset_text_box.ruby_unit,
        own.reset_text_box.ruby_unit || parent.reset_text_box.ruby_unit
    );
    set!(
        c.reset_text_box.ruby_suppress_breaks,
        own.reset_text_box.ruby_suppress_breaks || parent.reset_text_box.ruby_suppress_breaks
    );
    set!(
        c.reset_text_box.turned_line,
        own.reset_text_box.turned_line || parent.reset_text_box.turned_line
    );
    set!(
        c.reset_context.ortho_limit,
        own.reset_context
            .ortho_limit
            .or(layout.reset_context.ortho_limit)
    );
    set!(
        c.reset_context.orthogonal_scrollport,
        layout.reset_context.orthogonal_scrollport
    );
    // An absolutely positioned box sizes its inline axis against its own
    // containing block (CSS 2.1 §10.3.7 / §10.6.4 shrink-to-fit), never with
    // the in-flow inline measure of the paragraph it was written in.
    set!(
        c.reset_context.orthogonal_inline,
        if matches!(
            own.reset_positioned.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        ) {
            None
        } else {
            layout.reset_context.orthogonal_inline
        }
    );
    // A `-webkit-box` that is not a block container (no vertical orient)
    // is no line-clamp container and hands no clamp to its contents
    // (`line-clamp-029`).
    set!(
        c.reset_fragmentation.line_clamp,
        c.reset_fragmentation.line_clamp.or(layout
            .reset_fragmentation
            .line_clamp
            .filter(|_| layout.clamp_block_container()))
    );
    c.inline_decoration = c.inline_decoration.under(parent.inline_decoration);
    // css-ui-4 §6.1: `user-select` не наследуется; используемое значение
    // `auto` — `none` под коробкой с используемым `none`, иначе выделяемое
    // (`all` родителя тоже выделяемо).
    use crate::style::computed::UserSelect as U;
    set!(
        c.reset_context.no_select,
        match c.reset_context.user_select {
            Some(U::None) => Some(true),
            Some(U::Text | U::Contain | U::All) => Some(false),
            Some(U::Auto) | None => own
                .reset_context
                .no_select
                .or(parent.reset_context.no_select),
        }
    );
}
