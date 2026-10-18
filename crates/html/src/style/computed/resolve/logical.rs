//! Logical properties onto physical fields (css-logical-1 §4, css-writing-modes-4 §6.4):
//! `resolve_logical` maps the stored `Logical` values with the box's inherited `WritingMode`.
//!
//! Sides (margins, paddings, insets, borders) follow the abstract-physical mapping exactly,
//! through `WritingMode::{block_start, inline_start}` — the one mapping of the crate.
//! Sizes: `inline-size`/`block-size` map by the box's OWN mode (Blink
//! `ComputedStyle::LogicalWidth`), except for a table cell whose table is horizontal: the
//! table code transposes a cell's logical inline size itself (`width_from_inline`), and
//! mapping it here too would turn it twice. `contain-intrinsic-*` always maps by the own mode.

use super::*;
use crate::style::computed::writing_mode::{Side, WritingMode};

/// Physical side index of `Sides`-like arrays: top, right, bottom, left.
fn index(side: Side) -> u8 {
    match side {
        Side::Top => 0,
        Side::Right => 1,
        Side::Bottom => 2,
        Side::Left => 3,
    }
}

impl Computed {
    /// Место под логические значения — заводится по первому обращению: у
    /// подавляющего большинства узлов их нет вовсе.
    pub(crate) fn logical(&mut self) -> &mut Logical {
        self.logical.get_or_insert_with(Default::default)
    }

    /// Map the stored logical values with `wm`, the box's inherited writing mode;
    /// `parent_vertical` is the parent's (for the table-cell exception above).
    pub fn resolve_logical(&mut self, wm: WritingMode, parent_vertical: bool, is_cell: bool) {
        let Some(logical) = self.logical.take() else {
            return;
        };
        // A logical value OVERRIDES the physical one only when declared later
        // (cascade order; `line-box-direction-vrl-019`): `set` writes declared values.
        let set = |slot: &mut Option<Len>, val: Option<Len>| {
            if val.is_some() {
                *slot = val;
            }
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
        if wm.vertical() && (parent_vertical || !is_cell) {
            set(&mut self.height, logical.inline_size);
            set(&mut self.width, logical.block_size);
            set(&mut self.min_height, logical.min_inline);
            set(&mut self.min_width, logical.min_block);
            set(&mut self.max_height, logical.max_inline);
            set(&mut self.max_width, logical.max_block);
        } else {
            if wm.vertical() && logical.inline_size.is_some() {
                self.width_from_inline = true;
            }
            set(&mut self.width, logical.inline_size);
            set(&mut self.height, logical.block_size);
            set(&mut self.min_width, logical.min_inline);
            set(&mut self.min_height, logical.min_block);
            set(&mut self.max_width, logical.max_inline);
            set(&mut self.max_height, logical.max_block);
        }
        let (b_start, i_start) = (wm.block_start(), wm.inline_start());
        let [bs, ie, be, is] =
            [b_start, i_start.opposite(), b_start.opposite(), i_start].map(index);
        let phys_seq = self.side_seq;
        for (which, from, pseq) in [
            (0, &logical.padding, phys_seq.padding),
            (1, &logical.margin, phys_seq.margin),
            (2, &logical.inset, phys_seq.inset),
        ] {
            let to = match which {
                0 => &mut self.padding,
                1 => &mut self.margin,
                _ => &mut self.inset,
            };
            for (side, val, lseq) in [
                (is, from.inline_start, from.seq[0]),
                (ie, from.inline_end, from.seq[1]),
                (bs, from.block_start, from.seq[2]),
                (be, from.block_end, from.seq[3]),
            ] {
                let slot = match side {
                    0 => &mut to.top,
                    1 => &mut to.right,
                    2 => &mut to.bottom,
                    _ => &mut to.left,
                };
                if lseq >= pseq[side as usize] {
                    set(slot, val);
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
                self.apply_border_shorthand(v, Some(side as usize));
            }
        }
        let phys = [bs, ie, be, is];
        for (side, prop, v) in &logical.border_long {
            let name = ["top", "right", "bottom", "left"][phys[*side as usize] as usize];
            self.apply_one(&format!("border-{name}-{prop}"), v);
        }
    }
}
