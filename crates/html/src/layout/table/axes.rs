//! Оси таблицы: индексы сетки логические, а рамки и коробки раскладки физические.
//! `Axes` переводит стороны между ними по режиму письма и направлению таблицы, как Blink
//! `TableBorders::MergeBorders`; `inherited` доводит письмо родителя до стиля таблицы.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::style::computed::Computed;

#[derive(Clone, Copy)]
pub(super) struct Axes([usize; 4]);

impl Axes {
    /// Block start, inline end, block end, inline start, in physical side indices.
    pub(super) fn new(style: &Computed) -> Self {
        let wm = style.wm().used_direction();
        let (block, inline) = (wm.block_start(), wm.inline_start());
        Self([block, inline.opposite(), block.opposite(), inline].map(|side| side.index()))
    }

    pub(super) fn logical<T: Copy>(self, physical: [T; 4]) -> [T; 4] {
        self.0.map(|side| physical[side])
    }

    pub(super) fn physical<T: Copy>(self, logical: [T; 4]) -> [T; 4] {
        let mut physical = logical;
        for (side, value) in self.0.into_iter().zip(logical) {
            physical[side] = value;
        }
        physical
    }

    pub(super) fn mask(self, widths: [f32; 4], active: [bool; 4]) -> [f32; 4] {
        let widths = self.logical(widths);
        self.physical(std::array::from_fn(
            |i| if active[i] { widths[i] } else { 0.0 },
        ))
    }
}

pub(super) fn inherited(e: &Element, parent: &Computed, vcx: ValueCx<'_>) -> Option<Element> {
    let style = crate::style::cascade::materialize::box_style(parent, &e.style, vcx);
    if (
        e.style.writing_mode,
        e.style.flat_mode,
        e.style.rtl,
        e.style.text_orientation,
    ) == (
        style.writing_mode,
        style.flat_mode,
        style.rtl,
        style.text_orientation,
    ) {
        return None;
    }
    let mut copy = e.clone();
    copy.style.writing_mode = style.writing_mode;
    copy.style.flat_mode = style.flat_mode;
    copy.style.rtl = style.rtl;
    copy.style.text_orientation = style.text_orientation;
    Some(copy)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logical_table_edges_follow_the_grid_progression() {
        for (vertical, rl, sideways, rtl, expected) in [
            (false, false, false, false, [0, 1, 2, 3]),
            (false, false, false, true, [0, 3, 2, 1]),
            (true, true, false, false, [1, 2, 3, 0]),
            (true, false, false, false, [3, 2, 1, 0]),
            (true, false, true, false, [3, 0, 1, 2]),
            (true, true, false, true, [1, 0, 3, 2]),
        ] {
            let axes = Axes::new(&Computed {
                writing_mode: Some(crate::style::computed::writing_mode::WritingModeKw::new(
                    vertical, rl, sideways,
                )),
                rtl: Some(rtl),
                ..Computed::default()
            });
            assert_eq!(axes.logical([0, 1, 2, 3]), expected);
            assert_eq!(
                axes.physical(axes.logical([10, 20, 30, 40])),
                [10, 20, 30, 40]
            );
        }
    }
}
