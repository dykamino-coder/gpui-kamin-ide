//! Внешний вид полос прокрутки HTML по умолчанию: геометрия дорожки и скруглённого ползунка
//! по текущему смещению и диапазону, покраска с учётом `opacity` и фильтров. CSS Scrollbars 1
//! оставляет платформенный вид за UA; авторские `scrollbar-color` уводят на классическую тему.
//! Размеры классического gutter остаются в `scrollbar_gutter`.
use crate::style::computed::{BoxStyle, Computed};
use gpui::{Bounds, Pixels, ScrollHandle, Window, fill, point, px, size};

#[derive(Clone, Copy)]
pub(super) struct Bar {
    pub axis: usize,
    pub lane: Bounds<Pixels>,
    pub thumb: Bounds<Pixels>,
    pub travel: f32,
    pub maximum: f32,
    pub reverse: bool,
}

pub(super) fn bars(
    c: &Computed,
    handle: &ScrollHandle,
    selected: [bool; 2],
    range: [f32; 2],
    insets: [f32; 4],
    bounds: Bounds<Pixels>,
) -> Vec<Bar> {
    let width = c.scroll.thickness();
    if !super::uses_modern(c) || width == 0.0 || c.hidden == Some(true) {
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
            let available = (length - 6.0).max(0.0);
            let thumb_len = (available * length / (length + range[axis]).max(1.0))
                .max(width)
                .min(available);
            let travel = (available - thumb_len).max(0.0);
            let start = -f32::from(if axis == 0 { offset.x } else { offset.y });
            let start = if reverse[axis] {
                range[axis] - start
            } else {
                start
            };
            let position = 3.0 + travel * (start / range[axis].max(1.0)).clamp(0.0, 1.0);
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
            }
        })
        .collect()
}

pub(super) fn paint(c: &BoxStyle, bars: &[Bar], alpha: f32, window: &mut Window) {
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
        color.a *= c.opacity.unwrap_or(1.0);
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
            window.paint_quad(quad);
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
