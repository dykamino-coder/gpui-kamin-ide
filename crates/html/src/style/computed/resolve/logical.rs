//! Логические свойства на физические поля (css-logical-1 §4, css-writing-modes-4 §6.4):
//! `resolve_logical` раскладывает сохранённые `Logical` по унаследованному `WritingMode` коробки.
//! Логическое и физическое свойство одной стороны делят вычисленное значение, и побеждает
//! более сильное в каскаде объявление (Blink `StyleCascade` кладёт логическое свойство в ячейку
//! физического по письму, Stylo `LonghandId::to_physical`): сравниваются их `DeclOrder`.
//!
//! Стороны (поля, отступы, вставки, рамки) переводятся точно по abstract-physical mapping через
//! `WritingMode::{block_start, inline_start}` — единственное такое отображение в крейте.
//! Размеры: `inline-size`/`block-size` идут по СОБСТВЕННОМУ письму коробки (Blink
//! `ComputedStyle::LogicalWidth`), кроме ячейки таблицы с горизонтальной таблицей: строчный
//! размер ячейки табличный код переставляет сам (`width_from_inline`), и второй перевод здесь
//! повернул бы его дважды. `contain-intrinsic-*` всегда идёт по собственному письму.

use super::*;
use crate::document::state::ValueCx;
use crate::style::computed::props::box_model::side_slot;
use crate::style::computed::writing_mode::WritingMode;
use crate::style::css::BoxGroup;

impl Computed {
    /// Место под логические значения — заводится по первому обращению: у
    /// подавляющего большинства узлов их нет вовсе.
    pub(crate) fn logical(&mut self) -> &mut Logical {
        self.logical.get_or_insert_with(Default::default)
    }

    /// Разложить сохранённые логические значения по `wm` — унаследованному письму
    /// коробки; `parent_vertical` — письмо родителя (для исключения ячейки таблицы выше).
    ///
    /// Зовётся ПОСЛЕ того, как письмо унаследовано: до этого неизвестно, какая ось строчная.
    pub fn resolve_logical(
        &mut self,
        wm: WritingMode,
        parent_vertical: bool,
        is_cell: bool,
        vcx: ValueCx<'_>,
    ) {
        let Some(logical) = self.logical.take() else {
            return;
        };
        let set_ci = |slot: &mut Option<f32>, val: Option<f32>| {
            if val.is_some() {
                *slot = val;
            }
        };
        if wm.vertical() {
            set_ci(&mut self.contain_intrinsic.1, logical.ci_inline);
            set_ci(&mut self.contain_intrinsic.0, logical.ci_block);
        } else {
            set_ci(&mut self.contain_intrinsic.0, logical.ci_inline);
            set_ci(&mut self.contain_intrinsic.1, logical.ci_block);
        }
        // Логический размер i (inline, block, min-inline, min-block,
        // max-inline, max-block) → физический (width, height, min-width,
        // min-height, max-width, max-height). Строчный размер ячейки в
        // горизонтальной таблице табличный код переставляет сам.
        let rotate = wm.vertical() && (parent_vertical || !is_cell);
        for i in 0..6 {
            let Some(order) = logical.size_order[i] else {
                continue;
            };
            let phys = if rotate { i ^ 1 } else { i };
            if order < self.side_order.size[phys] {
                continue;
            }
            if i == 0 && wm.vertical() && !rotate {
                self.width_from_inline = true;
            }
            let value = [
                logical.inline_size,
                logical.block_size,
                logical.min_inline,
                logical.min_block,
                logical.max_inline,
                logical.max_block,
            ][i];
            self.set_physical_size(phys, value, logical.size_inherit[i]);
        }
        let (b_start, i_start) = (wm.block_start(), wm.inline_start());
        let [bs, ie, be, is] =
            [b_start, i_start.opposite(), b_start.opposite(), i_start].map(|s| s.index() as u8);
        for (group, from) in [
            (BoxGroup::Padding, &logical.padding),
            (BoxGroup::Margin, &logical.margin),
            (BoxGroup::Inset, &logical.inset),
        ] {
            for (lside, side) in [is, ie, bs, be].into_iter().enumerate() {
                let side = side as usize;
                let Some(order) = from.order[lside] else {
                    continue;
                };
                let (sides, flags, orders) = match group {
                    BoxGroup::Padding => (
                        &mut self.padding,
                        &mut self.padding_inherit_side,
                        &self.side_order.padding,
                    ),
                    BoxGroup::Margin => (
                        &mut self.margin,
                        &mut self.margin_inherit,
                        &self.side_order.margin,
                    ),
                    _ => (
                        &mut self.inset,
                        &mut self.inset_inherit,
                        &self.side_order.inset,
                    ),
                };
                if order < orders[side] {
                    continue;
                }
                flags[side] = from.inherit[lside];
                if let Some(v) = from.get(lside) {
                    *side_slot(sides, side) = Some(v);
                }
            }
        }
        // Logical border shorthands/longhands go through the ordinary physical parser.
        let borders = logical.border.clone();
        for (raw, side) in [
            (&borders[0], bs),
            (&borders[1], ie),
            (&borders[2], be),
            (&borders[3], is),
        ] {
            if let Some(v) = raw {
                self.apply_border_shorthand(v, Some(side as usize), vcx);
            }
        }
        let phys = [bs, ie, be, is];
        for (side, prop, v) in &logical.border_long {
            let name = ["top", "right", "bottom", "left"][phys[*side as usize] as usize];
            self.apply_one(&format!("border-{name}-{prop}"), v, vcx);
        }
    }

    /// Физический размер `i` (width, height, min-width, min-height,
    /// max-width, max-height) получает значение логического: прежние
    /// пометки физического объявления (`stretch`, `fit-content(N)`,
    /// `calc-size()`, `inherit`) уходят вместе с ним.
    fn set_physical_size(&mut self, i: usize, value: Option<Len>, inherit: bool) {
        self.stretch_size[[0, 1, 2, 4, 3, 5][i]] = false;
        self.calc_size_basis[i] = None;
        match i {
            0..4 => self.calc_size[i] = None,
            _ => self.calc_size_max[i - 4] = None,
        }
        match i {
            0 => self.fit_arg[0] = None,
            2 => self.fit_arg[1] = None,
            4 => self.fit_arg[2] = None,
            _ => {}
        }
        match i {
            0 => self.width_inherit = inherit,
            1 => self.height_inherit = inherit,
            _ => self.minmax_inherit[i - 2] = inherit,
        }
        if inherit {
            return;
        }
        *match i {
            0 => &mut self.width,
            1 => &mut self.height,
            2 => &mut self.min_width,
            3 => &mut self.min_height,
            4 => &mut self.max_width,
            _ => &mut self.max_height,
        } = value;
    }
}
