//! Внешний вид полос прокрутки HTML по умолчанию: геометрия дорожки и скруглённого ползунка
//! по текущему смещению и диапазону, покраска с учётом `opacity` и фильтров. CSS Scrollbars 1
//! оставляет платформенный вид за UA; авторские `scrollbar-color` уводят на классическую тему.
//! Размеры классического gutter остаются в `scrollbar_gutter`.
use crate::style::computed::{BoxStyle, Computed};
use gpui::{Bounds, Pixels, Point, ScrollHandle, Window, fill, point, px, size};

#[derive(Clone, Copy)]
pub(super) struct Bar {
    pub axis: usize,
    pub lane: Bounds<Pixels>,
    pub thumb: Bounds<Pixels>,
    pub travel: f32,
    pub maximum: f32,
    pub reverse: bool,
    pub button: f32,
    pub scale: f32,
    pub classic: bool,
}

pub(super) fn bars(
    c: &Computed,
    handle: &ScrollHandle,
    selected: [bool; 2],
    range: [f32; 2],
    insets: [f32; 4],
    bounds: Bounds<Pixels>,
    scale: f32,
) -> Vec<Bar> {
    let width = c.reset_box_layout.scroll.thickness();
    if width == 0.0 || c.inh.ui.hidden == Some(true) {
        return Vec::new();
    }
    let edge = |v| match v {
        Some(crate::style::values::value::Len::Px(v)) => v,
        _ => 0.0,
    };
    let edges = c.borders();
    let border = [
        edge(edges.left),
        edge(edges.top),
        edge(edges.right),
        edge(edges.bottom),
    ];
    let b = Bounds::new(
        bounds.origin + point(px(border[0]), px(border[1])),
        bounds.size - size(px(border[0] + border[2]), px(border[1] + border[3])),
    );
    let reverse = super::super::scrollbar_gutter::reversed(c);
    let offset = handle.offset();
    let selected = super::visible(c, selected);
    (0..2)
        .filter(|&axis| selected[axis])
        .map(|axis| {
            let length = if axis == 0 {
                f32::from(b.size.width) - insets[0] - insets[2]
            } else {
                f32::from(b.size.height) - insets[1] - insets[3]
            }
            .max(0.0);
            let lane = if axis == 0 {
                Bounds::new(
                    point(b.left() + px(insets[0]), b.bottom() - px(width)),
                    size(px(length), px(width)),
                )
            } else {
                let left = if super::super::scrollbar_gutter::on_left(c) {
                    b.left()
                } else {
                    b.right() - px(width)
                };
                Bounds::new(
                    point(left, b.top() + px(insets[1])),
                    size(px(width), px(length)),
                )
            };
            let button = if super::uses_modern(c) {
                3.0
            } else {
                width.min(length / 2.0)
            };
            let available = (length - 2.0 * button).max(0.0);
            let thumb_len = (available * length / (length + range[axis]).max(1.0))
                .max(width)
                .min(available);
            let travel = (available - thumb_len).max(0.0);
            let position =
                button + travel_at(axis, travel, range[axis], reverse[axis], offset, scale);
            let thumb = if axis == 0 {
                Bounds::new(
                    point(lane.left() + px(position), lane.top() + px(3.0)),
                    size(px(thumb_len), px(width - 6.0)),
                )
            } else {
                Bounds::new(
                    point(lane.left() + px(3.0), lane.top() + px(position)),
                    size(px(width - 6.0), px(thumb_len)),
                )
            };
            Bar {
                axis,
                lane,
                thumb,
                travel,
                maximum: range[axis],
                reverse: reverse[axis],
                button,
                scale,
                classic: !super::uses_modern(c),
            }
        })
        .collect()
}

/// How far along its travel a thumb lies at scroll offset `offset`.
fn travel_at(
    axis: usize,
    travel: f32,
    maximum: f32,
    reverse: bool,
    offset: Point<Pixels>,
    scale: f32,
) -> f32 {
    let start = -f32::from(if axis == 0 { offset.x } else { offset.y });
    let start = if reverse { maximum - start } else { start };
    // Native thumbs lie on the device grid, as does their retained displacement.
    (travel * (start / maximum.max(1.0)).clamp(0.0, 1.0) * scale).round() / scale
}

/// Prepaint each thumb as a follower of the scroller's node: a scrolled
/// replay moves it along its lane by the offset's change, as cc's
/// compositor places a scrollbar thumb from the scroll offset.
pub(super) fn follow(bars: &[Bar], handle: &ScrollHandle, window: &mut Window) {
    let base = handle.offset();
    for bar in bars {
        let Bar {
            axis,
            travel,
            maximum,
            reverse,
            scale,
            ..
        } = *bar;
        let from = travel_at(axis, travel, maximum, reverse, base, scale);
        let source = handle.clone();
        let along = move |_: Point<Pixels>| {
            // Content snaps to device pixels; the thumb follows the CSS offset.
            let by = px(travel_at(axis, travel, maximum, reverse, source.offset(), scale) - from);
            if axis == 0 {
                point(by, px(0.0))
            } else {
                point(px(0.0), by)
            }
        };
        let id = gpui::ScrollNodeId::follower(handle, axis);
        if maximum > 0.0 {
            window.with_scroll_follower_prepaint(id, Some(handle), std::rc::Rc::new(along), |_| {});
        }
        if bar.classic {
            super::buttons::follow(bar, handle, window);
        }
    }
}

pub(super) fn paint(
    c: &BoxStyle,
    bars: &[Bar],
    handle: &ScrollHandle,
    alpha: f32,
    window: &mut Window,
) {
    let color = |value: u32, alpha| {
        let mut color = crate::style::values::value::Color {
            r: value as f32 / 255.0,
            g: value as f32 / 255.0,
            b: value as f32 / 255.0,
            a: alpha,
        };
        for filter in c.paint_filter_ops() {
            color = filter.apply(color);
        }
        color.a *= c.reset_effects.opacity.unwrap_or(1.0);
        color.to_hsla()
    };
    for bar in bars {
        window.paint_quad(fill(bar.lane, color(248, 1.0)));
        if bar.maximum > 0.0 {
            let thickness = if bar.axis == 0 {
                bar.thumb.size.height
            } else {
                bar.thumb.size.width
            };
            let mut quad = fill(bar.thumb, color(110, alpha));
            quad.corner_radii = gpui::Corners::all(thickness / 2.0);
            let id = gpui::ScrollNodeId::follower(handle, bar.axis);
            window.with_scroll_node_paint(id, |window| window.paint_quad(quad));
        }
    }
    if bars.len() == 2 {
        let vertical = &bars[1];
        let horizontal = &bars[0];
        window.paint_quad(fill(
            Bounds::new(
                point(vertical.lane.left(), horizontal.lane.top()),
                size(vertical.lane.size.width, horizontal.lane.size.height),
            ),
            color(248, 1.0),
        ));
    }
}
