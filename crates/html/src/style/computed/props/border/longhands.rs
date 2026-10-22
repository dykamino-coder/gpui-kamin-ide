//! Длинные свойства рамки и контура по реестру: толщина, стиль и цвет каждой
//! физической стороны, логические кромки, `outline-*` и части `border-image`.
//! Значение пишется целиком или никак, `inherit` — признаком наследования
//! этого свойства; чужих свойств обработчик не читает. Правило «толщина
//! без рисунка — ноль» — используемое значение (css-backgrounds-3 §4.3), его
//! решает `Computed::borders`, а вычисленная толщина остаётся заданной.

use super::values::{border_style_rank, line_width, outline_line_style, outline_offset};
use crate::document::state::ValueCx;
use crate::style::computed::props::box_model::side_slot;
use crate::style::computed::{Computed, inh};
use crate::style::css::{BoxGroup, Grammar, LonghandId, Slot};
use crate::style::values::value::{ColorOrAuto, StyleColor};

impl Computed {
    /// Длинное свойство рамки или контура; `false` — свойство не отсюда.
    pub(super) fn apply_border_longhand(&mut self, key: &str, v: &str, vcx: ValueCx<'_>) -> bool {
        let Some(id) = LonghandId::from_name(key) else {
            return false;
        };
        match id.slot() {
            Slot::Physical(
                group @ (BoxGroup::BorderWidth | BoxGroup::BorderStyle | BoxGroup::BorderColor),
                side,
            ) => self.apply_border_side(part(group), side, v, vcx),
            Slot::Logical(
                group @ (BoxGroup::BorderWidth | BoxGroup::BorderStyle | BoxGroup::BorderColor),
                i,
            ) => {
                // Логическая кромка ждёт письма: на физическую сторону её
                // кладёт `resolve_logical` с местом объявления в каскаде.
                let order = self.decl_order;
                self.logical().border[part(group)][i] = Some((order, v.to_string()));
            }
            Slot::Single if id.grammar() == Grammar::BorderImage => {
                self.set_border_image(key, v, vcx)
            }
            Slot::Single => self.apply_outline_longhand(id, v, vcx),
            _ => return false,
        }
        true
    }

    /// Часть `part` (0 толщина, 1 стиль, 2 цвет) физической стороны `side`
    /// (top, right, bottom, left).
    fn apply_border_side(&mut self, part: usize, side: usize, v: &str, vcx: ValueCx<'_>) {
        let inherit = v.trim() == "inherit";
        match part {
            0 => {
                let width = line_width(v, vcx);
                if !inherit && width.is_none() {
                    return;
                }
                self.border_inherit_w[side] = inherit;
                if width.is_some() {
                    *side_slot(&mut self.border_width, side) = width;
                }
            }
            1 => {
                let rank = border_style_rank(v);
                if !inherit && rank.is_none() {
                    return;
                }
                self.border_inherit_s[side] = inherit;
                if let Some(rank) = rank {
                    self.border_side_styles[side] = Some(rank);
                    self.border_visible[side] = Some(rank >= 3);
                    self.sync_border_pattern();
                }
            }
            _ => {
                let color = StyleColor::parse_value(v, vcx);
                if !inherit && color.is_none() {
                    return;
                }
                self.border_inherit_c[side] = inherit;
                if let Some(color) = color {
                    self.border_colors[side] = color;
                }
            }
        }
        self.side_order.border[part][side] = self.decl_order;
    }

    /// Вычисленное значение начальной толщины `medium` — 3px
    /// (css-backgrounds-3 §4.3): после каскада видимая сторона без заданной
    /// толщины получает его явно (пустая толщина дальше значит «рамки нет»:
    /// так раскладка гасит сторону куска).
    pub(crate) fn settle_border_widths(&mut self) {
        let w = &mut self.border_width;
        for (side, visible) in [&mut w.top, &mut w.right, &mut w.bottom, &mut w.left]
            .into_iter()
            .zip(self.border_visible)
        {
            if visible == Some(true) {
                side.get_or_insert(crate::style::values::value::Len::Px(3.0));
            }
        }
    }

    /// Узор рамки целиком (квад GPUI знает один узор на элемент): общий
    /// стиль видимых сторон (`border-right: 10px dotted` — точки), а при
    /// разных стилях — стиль верхней стороны.
    pub(crate) fn sync_border_pattern(&mut self) {
        let mut visible = self
            .border_side_styles
            .iter()
            .flatten()
            .filter(|r| **r >= 3);
        let first = visible.next().copied();
        let shared = first.filter(|f| visible.all(|r| r == f));
        let rank = shared.or(self.border_side_styles[0]);
        self.border_dashed = rank.map(|r| r == 8);
        self.border_dotted = rank.map(|r| r == 7);
    }

    /// `outline-width`, `-style`, `-color`, `-offset`: одна часть записи
    /// контура.
    fn apply_outline_longhand(&mut self, id: LonghandId, v: &str, vcx: ValueCx<'_>) {
        let bit = match id {
            LonghandId::OutlineWidth => inh::OUTLINE_W,
            LonghandId::OutlineStyle => inh::OUTLINE_S,
            LonghandId::OutlineColor => inh::OUTLINE_C,
            _ => inh::OUTLINE_O,
        };
        if v.trim() == "inherit" {
            self.inherit_bits |= bit;
            return;
        }
        let mut o = self.outline.clone().unwrap_or_default();
        match id {
            LonghandId::OutlineWidth => match line_width(v, vcx) {
                Some(w) => o.width = Some(w),
                None => return,
            },
            LonghandId::OutlineStyle => match outline_line_style(v) {
                Some(s) => o.style = Some(s),
                None => return,
            },
            LonghandId::OutlineColor => match ColorOrAuto::parse_value(v, vcx) {
                Some(c) => o.color = c,
                None => return,
            },
            _ => match outline_offset(v, vcx) {
                Some((offset, inset)) => (o.offset, o.inset) = (offset, inset),
                None => return,
            },
        }
        self.inherit_bits &= !bit;
        self.outline = Some(o);
    }
}

/// Часть рамки по группе: 0 толщина, 1 стиль, 2 цвет.
fn part(group: BoxGroup) -> usize {
    match group {
        BoxGroup::BorderWidth => 0,
        BoxGroup::BorderStyle => 1,
        _ => 2,
    }
}
