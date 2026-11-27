//! Стадия коробки, содержащий блок: признаки cb_* (многоколонность, подложка, трансформ, письмо),
//! растяжение и определённость высоты. Родитель читается только через `ParentFacts`.

use super::*;
use crate::style::computed::BoxStyle;
mod identity;
pub(crate) use identity::inherit as reidentify;

pub(super) fn inherit_containing(f: &ParentFacts<'_>, own: &Computed, c: &mut BoxStyle) {
    let parent = &f.layout;
    // Исключение из `::first-letter` идёт по дереву элементов (потомки маркера).
    let excluded = own.reset_text_box.first_letter_excluded || f.tree.first_letter_excluded;
    set!(c.reset_text_box.first_letter_excluded, excluded);
    c.cb_ancestor = parent.cb_ancestor;
    c.cb_depth = parent.cb_depth;
    c.sc_ancestor = parent.sc_ancestor;
    c.cb_scroll_insets = parent.cb_scroll_insets;
    c.tree.fragment_clip_ancestor = parent.fragment_clip_ancestor;
    c.tree.in_multicol = parent.in_multicol;
    c.tree.backdrop_root_above = parent.backdrop_root_above;
    // Барьер содержащего блока для `position: fixed` — не только трансформ
    // (css-contain-1 §containment-layout п.1, §containment-paint;
    // `contain-layout-containing-block-fixed-001`).
    c.tree.transform_ancestor = parent.transform_ancestor;
    // css-transforms-1 §6: «Fixed backgrounds on the root element are affected
    // by any transform specified for that element. For all other elements that
    // are effected by a transform (i.e. have a transform applied to them, or to
    // any of their ancestor elements), a value of fixed for the
    // background-attachment property is treated as if it had a value of
    // scroll.» `will-change: transform` promises the same (Blink/Gecko;
    // `transform-fixed-bg-008.tentative`).
    c.tree.transformed_ancestor = parent.transformed_ancestor;
    if c.reset_background.bg_fixed == Some(true)
        && (c.tree.transformed_ancestor
            || (transformed(c) && !own.reset_context.root_box && !c.reset_context.root_box))
    {
        set!(c.reset_background.bg_fixed, Some(false));
    }
    // `will-change: z-index` — контекст наложения ровно там, где `z-index`
    // действует: у позиционированной коробки и у элемента flex/grid
    // (css-flexbox-1 §5.4, css-grid-2 §6.2) — `-stacking-context-z-index-2/3`;
    // у простого блока — нет (`-z-index-4` зелёный и обязан остаться).
    if own.reset_effects.will_change & crate::style::computed::wc::STACK_Z != 0
        && (matches!(
            own.reset_positioned.position,
            Some(crate::style::computed::Position::Relative)
                | Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
                | Some(crate::style::computed::Position::Sticky)
        ) || parent.flex_or_grid)
    {
        c.reset_effects.will_change |= crate::style::computed::wc::STACK;
    }
    // Относительный сдвиг строчного предка КОПИТСЯ вниз (§9.4.3: сдвиг несёт
    // с собой всё содержимое коробки). Куски вне потока его получали
    // (`shift_overlays`), а вложенные куски самой строки — нет: сдвиг
    // родителя терялся на первом же слиянии стилей.
    if c.reset_positioned.rel_shift.is_none() {
        set!(c.reset_positioned.rel_shift, parent.rel_shift);
    }
    if c.reset_positioned.box_shift.is_none() {
        set!(c.reset_positioned.box_shift, parent.box_shift);
    }
    // Элемент гибкого контейнера, сетки и содержимое ячейки получают
    // определённую высоту от РАСКЛАДКИ (css-flexbox-1 §9.8 растяжение,
    // css-grid-2 дорожки, §17.5.3 ячейка). Растяжение передаётся дальше: у
    // растянутой коробки высота от полосы, и для её потомков блок определён
    // (`column-align-items-005`).
    c.stretched = parent.laid_out
        && !matches!(
            own.reset_box_layout.height,
            Some(crate::style::values::value::Len::Px(_))
        );
    // §10.5: доля высоты считается только от ОПРЕДЕЛЁННОЙ высоты содержащего блока.
    c.cb_height_def = parent.child_cb_height_def;
}

#[cfg(test)]
mod tests;
