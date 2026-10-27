//! Покраска полос прокрутки HTML (CSS Overflow 3 §5, CSS Scrollbars 1 §2–3): классические
//! полосы с дорожкой, кнопками и ползунком в цветах `scrollbar-color` или Windows по умолчанию,
//! либо современные накладные полосы без заданных цветов. GPUI даёт прокрутку, но покраску
//! полос намеренно оставляет клиенту.
mod buttons;
pub(crate) mod interaction;
mod modern;
#[cfg(test)]
mod theme;
use crate::style::computed::{BoxStyle, Computed, Overflow};
use crate::style::values::value::Color;
use gpui::{Bounds, Pixels, ScrollHandle, Window, fill, point, px, size};

pub(crate) fn uses_modern(c: &Computed) -> bool {
    c.used_scrollbar_colors().is_none()
}

pub(crate) fn visible(c: &Computed, selected: [bool; 2]) -> [bool; 2] {
    let overflow = [c.overflow_x, c.overflow_y];
    std::array::from_fn(|axis| overflow[axis] == Some(Overflow::Scroll) && selected[axis])
}

// The painter consumes the scroll geometry and the GPUI paint context together.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint(
    c: &BoxStyle,
    handle: &ScrollHandle,
    selected: [bool; 2],
    range: [f32; 2],
    insets: [f32; 4],
    bounds: Bounds<Pixels>,
    alpha: f32,
    window: &mut Window,
) {
    if uses_modern(c) {
        let bars = modern::bars(c, handle, selected, range, insets, bounds);
        modern::paint(c, &bars, alpha, window);
        return;
    }
    let bars = visible(c, selected);
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
    let paint_color = |mut color: Color| {
        for filter in c.paint_filter_ops() {
            color = filter.apply(color);
        }
        color.a *= c.opacity.unwrap_or(1.0);
        color.to_hsla()
    };
    let color = |r, g, b| Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    };
    let (thumb, track) = match c.used_scrollbar_colors() {
        Some([thumb, track]) => (paint_color(thumb), paint_color(track)),
        None => (
            paint_color(color(193, 193, 193)),
            paint_color(color(241, 241, 241)),
        ),
    };
    let arrows = [
        paint_color(color(163, 163, 163)),
        paint_color(color(80, 80, 80)),
    ];
    let offset = handle.offset();
    let reversed = super::scrollbar_gutter::reversed(c);
    for axis in 0..2 {
        if !bars[axis] {
            continue;
        }
        let (length, start, maximum) = if axis == 0 {
            (
                f32::from(b.size.width) - insets[0] - insets[2],
                -f32::from(offset.x),
                range[0],
            )
        } else {
            (
                f32::from(b.size.height) - insets[1] - insets[3],
                -f32::from(offset.y),
                range[1],
            )
        };
        let start = if reversed[axis] {
            maximum - start
        } else {
            start
        };
        let length = length.max(0.0);
        let lane = if axis == 0 {
            Bounds::new(
                point(b.left() + px(insets[0]), b.bottom() - px(width)),
                size(px(length), px(width)),
            )
        } else {
            Bounds::new(
                point(
                    if super::scrollbar_gutter::on_left(c) {
                        b.left()
                    } else {
                        b.right() - px(width)
                    },
                    b.top() + px(insets[1]),
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
            (arrows, c.used_scrollbar_colors().map(|_| thumb)),
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
                if super::scrollbar_gutter::on_left(c) {
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
