//! Native scrollbar paint: shared thumb geometry and retained end-button states.
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
    let overflow = [c.reset_box_layout.overflow_x, c.reset_box_layout.overflow_y];
    std::array::from_fn(|axis| overflow[axis] == Some(Overflow::Scroll) && selected[axis])
}

/// Retain thumb motion and conditional end-button paint in spatial nodes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn follow(
    c: &Computed,
    handle: &ScrollHandle,
    selected: [bool; 2],
    range: [f32; 2],
    insets: [f32; 4],
    bounds: Bounds<Pixels>,
    window: &mut Window,
) {
    let bars = modern::bars(
        c,
        handle,
        selected,
        range,
        insets,
        bounds,
        window.scale_factor(),
    );
    modern::follow(&bars, handle, window);
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
        // Each thumb paints under its follower node (`follow`).
        let bars = modern::bars(
            c,
            handle,
            selected,
            range,
            insets,
            bounds,
            window.scale_factor(),
        );
        modern::paint(c, &bars, handle, alpha, window);
        return;
    }
    let bars = visible(c, selected);
    let width = c.reset_box_layout.scroll.thickness();
    if width == 0.0 || c.inh.ui.hidden == Some(true) {
        return;
    }
    let paint_color = |mut color: Color| {
        for filter in c.paint_filter_ops() {
            color = filter.apply(color);
        }
        color.a *= c.reset_effects.opacity.unwrap_or(1.0);
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
    let geometry = modern::bars(
        c,
        handle,
        selected,
        range,
        insets,
        bounds,
        window.scale_factor(),
    );
    for bar in &geometry {
        window.paint_quad(fill(bar.lane, track));
        if bar.maximum > 0.0 {
            let mut rect = bar.thumb;
            // Classic thumbs leave two pixels across the lane.
            if bar.axis == 0 {
                rect.origin.y -= px(1.0);
                rect.size.height += px(2.0);
            } else {
                rect.origin.x -= px(1.0);
                rect.size.width += px(2.0);
            }
            window.with_scroll_node_paint(
                gpui::ScrollNodeId::follower(handle, bar.axis),
                |window| {
                    window.paint_quad(fill(rect, thumb));
                },
            );
        }
        buttons::paint(
            bar,
            handle,
            (arrows, c.used_scrollbar_colors().map(|_| thumb)),
            window,
        );
    }
    if bars == [true, true] {
        let vertical = &geometry[1];
        let horizontal = &geometry[0];
        window.paint_quad(fill(
            Bounds::new(
                point(vertical.lane.left(), horizontal.lane.top()),
                size(vertical.lane.size.width, horizontal.lane.size.height),
            ),
            track,
        ));
    }
}
