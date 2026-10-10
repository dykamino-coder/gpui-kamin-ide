//! Text decoration lines (css-text-decor-3 §2, css-text-decor-4 §2–§4).
//!
//! Geometry follows Blink `TextDecorationInfo` / `TextDecorationOffset` /
//! `DecorationLinePainter`, which work in device pixels (zoom-for-DSF): the
//! thickness is rounded there, the line is snapped vertically to the device
//! grid, and the run ends land on whole device pixels.

mod ink;
mod paint_decor;
mod shaped;

mod skip_spaces;

use super::*;
use crate::style::computed::{DecorLen, DecorStyle};
use gpui::{Bounds, Hsla, Window, point, px, size};

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x11FF | 0x2E80..=0x9FFF | 0xA960..=0xA97F | 0xAC00..=0xD7FF
        | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF | 0x20000..=0x3FFFF)
}

/// One decoration line in device px (Blink `DecorationGeometry`).
#[derive(Clone, Debug)]
pub struct DecorLine {
    style: DecorStyle,
    x0: f32,
    x1: f32,
    /// Top of the line rect, unsnapped.
    y: f32,
    t: f32,
    double_offset: f32,
    wavy_offset: f32,
    color: Hsla,
}

fn inset_zero(l: DecorLen) -> bool {
    match l {
        DecorLen::Px(v) | DecorLen::Pct(v) => v == 0.0,
        DecorLen::Mix(k, v) => k == 0.0 && v == 0.0,
        _ => true,
    }
}

/// Inset in device px; a percentage refers to `basis` (device px).
fn inset_px(l: DecorLen, basis: f32, scale: f32) -> f32 {
    match l {
        DecorLen::Px(v) => v * scale,
        DecorLen::Pct(k) => k * basis,
        DecorLen::Mix(k, v) => k * basis + v * scale,
        _ => 0.0,
    }
}

/// Blink `ResolveInsetForFragment`: with `slice`, a positive inset trims the
/// run from its start and spills over the following fragments, a negative one
/// extends only the outer end.
fn inset_for_fragment(inset: f32, preceding: f32) -> f32 {
    if inset <= 0.0 {
        if preceding == 0.0 { inset } else { 0.0 }
    } else {
        (inset - preceding).max(0.0)
    }
}

/// Rect in device px → logical quad.
fn dev_quad(x0: f32, x1: f32, y: f32, h: f32, color: Hsla, scale: f32, window: &mut Window) {
    if x1 <= x0 || h <= 0.0 {
        return;
    }
    window.paint_quad(gpui::fill(
        Bounds {
            origin: point(px(x0 / scale), px(y / scale)),
            size: size(px((x1 - x0) / scale), px(h / scale)),
        },
        color,
    ));
}

/// One decoration line (Blink `DecorationLinePainter::Paint`): `y` is the
/// top of the line rect in device px, `double_offset` the second line of
/// `double`, `wavy_offset` the shift of the wave.
#[allow(clippy::too_many_arguments)]
fn paint_line_style(
    style: DecorStyle,
    x0: f32,
    x1: f32,
    y: f32,
    t: f32,
    double_offset: f32,
    wavy_offset: f32,
    color: Hsla,
    scale: f32,
    window: &mut Window,
) {
    // `SnapYAxis`: nearest device row, thickness rounded down.
    let h = t.floor().max(1.0);
    match style {
        DecorStyle::Solid | DecorStyle::Double => {
            dev_quad(x0, x1, (y + 0.5).floor(), h, color, scale, window);
            if style == DecorStyle::Double {
                dev_quad(
                    x0,
                    x1,
                    (y + double_offset + 0.5).floor(),
                    h,
                    color,
                    scale,
                    window,
                );
            }
        }
        DecorStyle::Dotted | DecorStyle::Dashed => {
            // `DrawLineAsStroke`: the stroke centre on a device row.
            let ti = t.round().max(1.0);
            let mid = (y + (t / 2.0).max(0.5)).floor();
            let top = if ti as i32 % 2 == 1 {
                mid + 0.5 - ti / 2.0
            } else {
                mid - ti / 2.0
            };
            let (dash, gap) = if style == DecorStyle::Dashed {
                (3.0 * ti, 3.0 * ti)
            } else {
                (ti, ti)
            };
            let mut x = x0;
            while x < x1 {
                let end = (x + dash).min(x1);
                if style == DecorStyle::Dotted && ti >= 3.0 {
                    window.paint_quad(
                        gpui::fill(
                            Bounds {
                                origin: point(px(x / scale), px(top / scale)),
                                size: size(px((end - x) / scale), px(ti / scale)),
                            },
                            color,
                        )
                        .corner_radii(px(ti / 2.0 / scale)),
                    );
                } else {
                    dev_quad(x, end, top, ti, color, scale, window);
                }
                x += dash + gap;
            }
        }
        DecorStyle::Wavy => {
            // Blink `MakeWave`: the wave sits below an underline and above
            // an overline by the thickness plus one.
            let amp = (0.5 + (3.0 * t + 0.5).round()) * 0.29;
            let centre = y + 0.5 + wavy_offset;
            window.paint_underline(
                point(px(x0 / scale), px((centre - amp - t / 2.0) / scale)),
                px((x1 - x0) / scale),
                &gpui::UnderlineStyle {
                    thickness: px(t / scale),
                    color: Some(color),
                    wavy: true,
                },
            );
        }
    }
}

impl DecorLine {
    pub(crate) fn joins(&self, next: &DecorLine) -> bool {
        self.style == next.style
            && self.y == next.y
            && self.t == next.t
            && self.color == next.color
            && (next.x0 - self.x1).abs() < 0.01
    }
}

impl DecorLine {
    pub(crate) fn paint(&self, scale: f32, window: &mut Window) {
        paint_line_style(
            self.style,
            self.x0,
            self.x1,
            self.y,
            self.t,
            self.double_offset,
            self.wavy_offset,
            self.color,
            scale,
            window,
        );
    }
}
