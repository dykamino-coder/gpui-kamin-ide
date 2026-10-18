//! Physical classic gutters (CSS Overflow 3 §4.2), including stable and both-edges.
//! Grid axis reversal is not the Windows scrollbar-side policy.
pub(crate) mod positioned;
use crate::style::computed::{Computed, Display, Overflow};
use gpui::{AbsoluteLength, DefiniteLength, Div, Length, Styled, px};

#[derive(Clone, Copy)]
pub(crate) struct Model {
    pub bars: [bool; 2],
    /// Left, top, right, bottom, as painted on Windows.
    pub used: [f32; 4],
    /// Insets already reserved by Taffy's scrollbar_width.
    pub native: [f32; 4],
}
impl Model {
    pub(crate) fn new(c: &Computed, bars: [bool; 2]) -> Self {
        let width = c.scroll.thickness();
        let left = on_left(c);
        let grid = matches!(c.display, Some(Display::Grid | Display::InlineGrid));
        let native_left = grid && reversed(c)[0];
        let mut model = Self {
            bars,
            used: [0.0; 4],
            native: [0.0; 4],
        };
        if bars[0] {
            model.used[3] = width;
            model.native[3] = width;
        }
        if bars[1] {
            model.used[if left { 0 } else { 2 }] = width;
            model.native[if native_left { 0 } else { 2 }] = width;
        }
        let vertical = c.wm().vertical();
        let overflow = if vertical { c.overflow_x } else { c.overflow_y };
        if !c.scroll.viewport_root
            && c.scroll.gutter.is_some_and(|v| v != 0)
            && matches!(overflow, Some(Overflow::Hidden | Overflow::Scroll))
        {
            let side = if vertical {
                3
            } else if left {
                0
            } else {
                2
            };
            model.used[side] = width;
            if c.scroll.gutter == Some(2) {
                model.used[(side + 2) % 4] = width;
            }
        }
        model
    }
    pub(crate) fn total(self) -> [f32; 2] {
        [self.used[0] + self.used[2], self.used[1] + self.used[3]]
    }
    pub(crate) fn native_total(self) -> [f32; 2] {
        [
            self.native[0] + self.native[2],
            self.native[1] + self.native[3],
        ]
    }
}
fn add(value: DefiniteLength, delta: f32) -> DefiniteLength {
    match value {
        DefiniteLength::Absolute(AbsoluteLength::Pixels(v)) => px(f32::from(v) + delta).into(),
        // Resolve the HTML fallback padding using the document root font.
        DefiniteLength::Absolute(AbsoluteLength::Rems(v)) => {
            px(v.0 * crate::style::values::value::root_font_px() + delta).into()
        }
        DefiniteLength::Fraction(v) => DefiniteLength::Calc(delta, v),
        DefiniteLength::Calc(px, pct) => DefiniteLength::Calc(px + delta, pct),
    }
}
fn subtract(value: &mut Option<Length>, extra: f32) {
    if let Some(Length::Definite(v)) = value {
        *v = add(*v, -extra);
    }
}
pub(crate) fn apply(mut div: Div, c: &Computed) -> Div {
    let model = super::scroll_extent::classic::model(c.self_node, c);
    let style = div.style();
    let difference: [f32; 4] = std::array::from_fn(|side| model.used[side] - model.native[side]);
    for (side, extra) in [
        &mut style.padding.left,
        &mut style.padding.top,
        &mut style.padding.right,
        &mut style.padding.bottom,
    ]
    .into_iter()
    .zip(difference)
    {
        if extra != 0.0 {
            *side = Some(add(side.unwrap_or_else(|| px(0.0).into()), extra));
        }
    }
    if style.content_box == Some(true) {
        let used = model.total();
        let native = model.native_total();
        for (axis, extra) in [used[0] - native[0], used[1] - native[1]]
            .into_iter()
            .enumerate()
        {
            if extra == 0.0 {
                continue;
            }
            if axis == 0 {
                subtract(&mut style.size.width, extra);
                subtract(&mut style.min_size.width, extra);
                subtract(&mut style.max_size.width, extra);
            } else {
                subtract(&mut style.size.height, extra);
                subtract(&mut style.min_size.height, extra);
                subtract(&mut style.max_size.height, extra);
            }
        }
    }
    if model.used != [0.0; 4] {
        let mut clip = style.overflow_clip_offset.unwrap_or([0.0; 4]);
        for (index, side) in [1, 2, 3, 0].into_iter().enumerate() {
            clip[index] -= model.used[side];
        }
        style.overflow_clip_offset = Some(clip);
    }
    div
}

/// Physical axes whose CSS scroll origin is at their far edge.
pub(crate) fn reversed(c: &Computed) -> [bool; 2] {
    c.wm().reversed_axes()
}

/// Blink ComputedStyle::ShouldPlaceBlockDirectionScrollbarOnLogicalLeft.
pub(crate) fn on_left(c: &Computed) -> bool {
    !c.wm().vertical() && c.wm().rtl()
}
