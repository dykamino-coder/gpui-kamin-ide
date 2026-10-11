//! Classic HTML scrollbars: CSS Overflow 3 §5 and CSS Scrollbars 1 §2–3.
//! GPUI provides scrolling but deliberately leaves scrollbar painting to clients.
mod buttons;
use crate::style::computed::{Computed, Overflow};
use gpui::{Bounds, Pixels, ScrollHandle, Window, fill, point, px, rgb, size};

pub(crate) fn visible(c: &Computed, handle: &ScrollHandle) -> [bool; 2] {
    let max = handle.max_offset();
    let enabled = [
        c.overflow_x == Some(Overflow::Scroll),
        c.overflow_y == Some(Overflow::Scroll),
    ];
    [
        enabled[0] && (c.scroll.forced[0] || max.x > px(0.01)),
        enabled[1] && (c.scroll.forced[1] || max.y > px(0.01)),
    ]
}

pub(crate) fn paint(
    c: &Computed,
    handle: &ScrollHandle,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) {
    // Explicit CSS colors are the first supported scrollbar painting policy.
    // Automatic platform scrollbars await a viewport and intrinsic-sizing owner.
    if c.scroll.colors.flatten().is_none() {
        return;
    }
    let bars = visible(c, handle);
    let width = c.scroll.thickness();
    if width == 0.0 || c.hidden == Some(true) {
        return;
    }
    let edges = c.borders();
    let edge = |v| match v {
        Some(crate::style::values::value::Len::Px(v)) => v,
        _ => 0.0,
    };
    let mut b = bounds;
    b.origin.x += px(edge(edges.left));
    b.origin.y += px(edge(edges.top));
    b.size.width -= px(edge(edges.left) + edge(edges.right));
    b.size.height -= px(edge(edges.top) + edge(edges.bottom));
    let (thumb, track) = match c.scroll.colors.flatten() {
        Some([thumb, track]) => (thumb.to_hsla(), track.to_hsla()),
        None => (rgb(0xc1c1c1).into(), rgb(0xf1f1f1).into()),
    };
    let max = handle.max_offset();
    let offset = handle.offset();
    for axis in 0..2 {
        if !bars[axis] {
            continue;
        }
        let (length, other, start, maximum) = if axis == 0 {
            (
                f32::from(b.size.width),
                bars[1],
                -f32::from(offset.x),
                f32::from(max.x),
            )
        } else {
            (
                f32::from(b.size.height),
                bars[0],
                -f32::from(offset.y),
                f32::from(max.y),
            )
        };
        let length = (length - if other { width } else { 0.0 }).max(0.0);
        let lane = if axis == 0 {
            Bounds::new(
                point(
                    b.left()
                        + if other && c.rtl == Some(true) {
                            px(width)
                        } else {
                            px(0.0)
                        },
                    b.bottom() - px(width),
                ),
                size(px(length), px(width)),
            )
        } else {
            Bounds::new(
                point(
                    if c.rtl == Some(true) {
                        b.left()
                    } else {
                        b.right() - px(width)
                    },
                    b.top(),
                ),
                size(px(width), px(length)),
            )
        };
        window.paint_quad(fill(lane, track));
        // Classic scrollbars have an arrow button at either end. The thumb
        // moves only on the intervening track (Blink ScrollbarTheme geometry).
        let button = width.min(length / 2.0);
        buttons::paint(
            lane,
            axis,
            button,
            start,
            maximum,
            c.scroll.colors.flatten().map(|_| thumb),
            window,
        );
        let travel = (length - 2.0 * button).max(0.0);
        let view = length;
        let thumb_len = (travel * view / (view + maximum)).max(width).min(travel);
        let position = button
            + if maximum > 0.0 {
                (travel - thumb_len) * (start / maximum).clamp(0.0, 1.0)
            } else {
                0.0
            };
        if maximum > 0.0 {
            let rect = if axis == 0 {
                Bounds::new(
                    point(lane.left() + px(position), lane.top() + px(2.0)),
                    size(px(thumb_len), px(width - 4.0)),
                )
            } else {
                Bounds::new(
                    point(lane.left() + px(2.0), lane.top() + px(position)),
                    size(px(width - 4.0), px(thumb_len)),
                )
            };
            window.paint_quad(fill(rect, thumb));
        }
    }
    if bars == [true, true] {
        let corner = Bounds::new(
            point(
                if c.rtl == Some(true) {
                    b.left()
                } else {
                    b.right() - px(width)
                },
                b.bottom() - px(width),
            ),
            size(px(width), px(width)),
        );
        window.paint_quad(fill(corner, track));
    }
}
