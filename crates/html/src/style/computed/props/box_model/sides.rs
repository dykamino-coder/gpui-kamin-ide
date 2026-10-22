//! Стороны коробки по реестру длинных свойств: физические поля, отступы и
//! вставки (`margin-top`, `padding-left`, `top`) и логические стороны и
//! размеры (`margin-inline-start`, `inset-block-end`, `inline-size`). Значение
//! пишется целиком или никак; `inherit` — признаком наследования стороны.
//! Логические значения ждут письма в `Logical` с местом объявления в каскаде:
//! спор с физической стороной решает `resolve_logical` по этому месту.

use super::values::side_len;
use super::*;
use crate::document::state::ValueCx;
use crate::style::css::{BoxGroup, Grammar, LonghandId, Slot};

impl Computed {
    pub(super) fn apply_box_model_sides(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        let Some(id) = LonghandId::from_name(key) else {
            return self.apply_box_model_aspect(key, v, hit);
        };
        match id.slot() {
            Slot::Physical(
                group @ (BoxGroup::Margin | BoxGroup::Padding | BoxGroup::Inset),
                side,
            ) => self.apply_side(id.grammar(), group, side, v, vcx),
            Slot::Logical(
                group @ (BoxGroup::Margin | BoxGroup::Padding | BoxGroup::Inset | BoxGroup::Size),
                i,
            ) => self.apply_logical(id.grammar(), group, i, v, vcx),
            // Размеры решает `apply_box_model`, рамку и контур — их группа.
            _ => *hit = false,
        }
    }

    /// Физическая сторона `side` (top, right, bottom, left) группы.
    fn apply_side(
        &mut self,
        grammar: Grammar,
        group: BoxGroup,
        side: usize,
        v: &str,
        vcx: ValueCx<'_>,
    ) {
        // CSS 2.1 §6.2.1: `inherit` берёт вычисленную сторону родителя.
        let inherit = v == "inherit";
        let len = side_len(grammar, v, vcx);
        if !inherit && len.is_none() {
            return;
        }
        let order = self.decl_order;
        let (sides, flags, orders) = match group {
            BoxGroup::Margin => (
                &mut self.margin,
                &mut self.margin_inherit,
                &mut self.side_order.margin,
            ),
            BoxGroup::Padding => (
                &mut self.padding,
                &mut self.padding_inherit_side,
                &mut self.side_order.padding,
            ),
            _ => (
                &mut self.inset,
                &mut self.inset_inherit,
                &mut self.side_order.inset,
            ),
        };
        flags[side] = inherit;
        orders[side] = order;
        if let Some(len) = len {
            *side_slot(sides, side) = Some(len);
        }
    }

    /// Логическая сторона или размер: значение ждёт письма в `Logical`.
    fn apply_logical(
        &mut self,
        grammar: Grammar,
        group: BoxGroup,
        i: usize,
        v: &str,
        vcx: ValueCx<'_>,
    ) {
        let inherit = v == "inherit";
        let order = self.decl_order;
        if group == BoxGroup::Size {
            let mut parsed = None;
            assign_size(&mut parsed, v, vcx);
            let none = v.trim().eq_ignore_ascii_case("none");
            if !inherit && parsed.is_none() && !none {
                return;
            }
            let logical = self.logical();
            logical.size_order[i] = Some(order);
            logical.size_inherit[i] = inherit;
            if !inherit {
                *logical_size_slot(logical, i) = parsed;
            }
            return;
        }
        let len = side_len(grammar, v, vcx);
        if !inherit && len.is_none() {
            return;
        }
        let logical = self.logical();
        let target = match group {
            BoxGroup::Margin => &mut logical.margin,
            BoxGroup::Padding => &mut logical.padding,
            _ => &mut logical.inset,
        };
        target.order[i] = Some(order);
        target.inherit[i] = inherit;
        *target.slot(i) = len;
    }
}

/// Поле стороны (top, right, bottom, left).
pub(in crate::style::computed) fn side_slot(sides: &mut Sides, side: usize) -> &mut Option<Len> {
    match side {
        0 => &mut sides.top,
        1 => &mut sides.right,
        2 => &mut sides.bottom,
        _ => &mut sides.left,
    }
}

/// Поле логического размера (inline, block, min-inline, min-block,
/// max-inline, max-block).
fn logical_size_slot(logical: &mut Logical, i: usize) -> &mut Option<Len> {
    match i {
        0 => &mut logical.inline_size,
        1 => &mut logical.block_size,
        2 => &mut logical.min_inline,
        3 => &mut logical.min_block,
        4 => &mut logical.max_inline,
        _ => &mut logical.max_block,
    }
}

#[cfg(test)]
mod tests;
