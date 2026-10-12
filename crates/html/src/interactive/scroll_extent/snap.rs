//! Initial snapping after measured layout (CSS Scroll Snap 1 §4.1.3, §6.2).
//! Like Blink FragmentBuilder::ResolveSnapArea, the nearest scroll container
//! owns descendant snap areas and resolves their logical alignment axes.
use super::geometry::{Rect, sides};
use crate::style::computed::{Computed, Sides, snap::SnapStyle};
use crate::style::values::value::{Len, calc_get};
use gpui::{ScrollHandle, point, px};

#[derive(Clone, Copy)]
pub(super) struct Area {
    pub rect: Rect,
    pub snap: SnapStyle,
    pub vertical: bool,
    pub reverse: [bool; 2],
}

fn snap_sides(s: Sides, view: [f32; 2]) -> [f32; 4] {
    let mut values = [0.0; 4];
    for (i, v) in [s.left, s.top, s.right, s.bottom].into_iter().enumerate() {
        values[i] = match v {
            Some(Len::Px(n)) => n,
            Some(Len::Pct(n)) => n * view[i % 2],
            Some(Len::Calc(index)) => {
                let sum = calc_get(index);
                sum.px + sum.pct * view[i % 2]
            }
            _ => 0.0,
        };
    }
    values
}

pub(super) fn initial(
    c: &Computed,
    areas: impl Iterator<Item = Area>,
    handle: &ScrollHandle,
    bounds: gpui::Bounds<gpui::Pixels>,
    insets: [f32; 4],
    maximum: [f32; 2],
) -> bool {
    let border = sides(c.borders(), 0.0);
    let origin = [
        f32::from(bounds.left()) + border[0] + insets[0],
        f32::from(bounds.top()) + border[1] + insets[1],
    ];
    let view = [
        f32::from(bounds.size.width) - border[0] - border[2] - insets[0] - insets[2],
        f32::from(bounds.size.height) - border[1] - border[3] - insets[1] - insets[3],
    ];
    let padding = snap_sides(c.snap.padding, view);
    let vertical = c.vertical == Some(true);
    let reverse = super::super::scrollbar_gutter::reversed(c);
    let active = match c.snap.axis {
        1 => [true, false],
        2 => [false, true],
        3 => [vertical, !vertical],
        4 => [!vertical, vertical],
        5 => [true, true],
        _ => return false,
    };
    let offset = handle.offset();
    let current = [-f32::from(offset.x), -f32::from(offset.y)];
    let mut choices = [Vec::new(), Vec::new()];
    for area in areas {
        let margin = snap_sides(area.snap.margin, view);
        let rect = [
            area.rect[0] - margin[0],
            area.rect[1] - margin[1],
            area.rect[2] + margin[2],
            area.rect[3] + margin[3],
        ];
        for axis in 0..2 {
            if !active[axis] {
                continue;
            }
            let port_start = origin[axis] + padding[axis];
            let port_end = origin[axis] + view[axis] - padding[axis + 2];
            // Oversized snap areas resolve start/end against their own writing
            // direction, so their readable start stays reachable (§5.2).
            let oversized = rect[axis + 2] - rect[axis] > port_end - port_start;
            let mode = if oversized { area.vertical } else { vertical };
            let reversed = if oversized {
                area.reverse[axis]
            } else {
                reverse[axis]
            };
            let logical_axis = if mode == (axis == 0) { 0 } else { 1 };
            let align = area.snap.align[logical_axis];
            let delta = match (align, reversed) {
                (1, false) | (2, true) => rect[axis] - port_start,
                (2, false) | (1, true) => rect[axis + 2] - port_end,
                (3, _) => (rect[axis] + rect[axis + 2] - port_start - port_end) / 2.0,
                _ => continue,
            };
            let delta = if reverse[axis] { -delta } else { delta };
            let candidate = (current[axis] + delta).clamp(0.0, maximum[axis]);
            let distance = (candidate - current[axis]).abs();
            // Proximity is UA-defined (§6.2); use a fraction of the snapport,
            // rather than a fixed CSS-pixel threshold across differently sized ports.
            if c.snap.strictness == 1 && distance > view[axis] / 3.0 {
                continue;
            }
            choices[axis].push(selection::Choice {
                position: candidate,
                area: Some(std::array::from_fn(|side| {
                    let a = side % 2;
                    if reverse[a] {
                        2.0 * origin[a] + view[a] - rect[(side + 2) % 4]
                    } else {
                        rect[side]
                    }
                })),
            });
        }
    }
    let target = selection::choose(choices, current, origin, view);
    if (target[0] - current[0]).abs() < 0.01 && (target[1] - current[1]).abs() < 0.01 {
        return false;
    }
    handle.set_offset(point(px(-target[0]), px(-target[1])));
    true
}

mod selection;
