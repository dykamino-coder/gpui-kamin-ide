//! Статическая сторона inline overlay отделена от геометрии блочного щупа.

use crate::layout::positioned::containing_block::{LineBoxSlot, Spot, SpotCell, placed_bounds};
use gpui::{Bounds, LayoutSpace, Pixels, Point, Window, point, px};

pub(super) enum StaticPosition {
    Probe(SpotCell, Option<LineBoxSlot>),
    Inline {
        line: LineBoxSlot,
        rtl: bool,
        margin: Point<Pixels>,
    },
}

type Resolved = (Bounds<Pixels>, Option<Bounds<Pixels>>, Option<LayoutSpace>);

impl StaticPosition {
    pub(super) fn resolve(&self, bounds: Bounds<Pixels>, window: &Window) -> Option<Resolved> {
        match self {
            Self::Probe(cell, line) => {
                let spot = cell.get();
                if spot.fixed_axes == (true, true) {
                    return Some((bounds, None, None));
                }
                let (mut spot, space) = match line {
                    Some(line) => line.normal.get()?.read(spot, window)?,
                    None => spot.normal(window)?,
                };
                // В normal geometry точечный RTL-щуп отмечает inline-start
                // гипотетической коробки: CSS2 §10.3.7 разрешает от него right.
                if spot.rtl && !spot.vertical && !spot.rotated && !spot.fixed_axes.0 {
                    spot.block_strut = true;
                }
                Some((placed_bounds(bounds, spot), spot.hole, Some(space)))
            }
            Self::Inline { line, rtl, margin } => {
                let (spot, space) = line.normal.get()?.read(Spot::default(), window)?;
                let rect = spot.hole?;
                // CSS2 §10.3.7: inline-start принадлежит направлению родителя.
                // Это та же сторона, что у InlineStartHang, без paint translation.
                let origin = rect.origin
                    + *margin
                    + point(if *rtl { rect.size.width } else { px(0.) }, px(0.))
                    - point(if *rtl { bounds.size.width } else { px(0.) }, px(0.));
                Some((
                    Bounds {
                        origin,
                        size: bounds.size,
                    },
                    Some(rect),
                    Some(space),
                ))
            }
        }
    }
}
