//! The writing mode of a box as ONE value, and the one logical↔physical side mapping
//! (css-writing-modes-4 §3.1 `writing-mode`, §2.1 `direction`, §5.1 `text-orientation`, §6.4 Abstract-Physical Mapping).
//!
//! Every reader asks `Computed::wm()`; nothing else decodes the stored properties. The combined
//! value follows Blink's `ComputedStyle::GetWritingDirection()` (`core/style/computed_style.h`)
//! and Servo's `WritingMode` (`style/logical_geometry.rs`): writing-mode + direction in one copy
//! type with queries.
//!
//! Layout here is physical (GPUI/taffy boxes) and vertical text uses a rotation model: a vertical
//! paragraph is typeset by a horizontal clone of its style that is then turned
//! (`render/paragraph/vertical.rs`), and an upright stack is laid out the same way. Inside such a
//! clone the box is laid out horizontally ("laid flat") while its writing mode — block flow
//! direction and `sideways` — is still that of the paragraph. So the value distinguishes
//! `vertical()` (laid out vertically in the current coordinate space) from the mode's block flow
//! (`block_rl()`, `sideways()`), and records whether the box sits in a turned line (`turned()`).
use super::Computed;

mod keyword;
mod props;
mod side;
pub(crate) use keyword::{TextOrientation, WritingModeKw};
pub(crate) use props::WritingProps;
pub(crate) use side::{Side, Turn};

/// The used writing mode and inline base direction of a box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WritingMode {
    vertical: bool,
    /// Block flow right-to-left (`vertical-rl`, `sideways-rl`), also when laid flat.
    rl: bool,
    /// `sideways-*`, also when laid flat.
    sideways: bool,
    /// Inside the horizontal clone of a turned vertical paragraph.
    turned: bool,
    rtl: bool,
    orientation: TextOrientation,
}

impl WritingMode {
    /// A box laid out in `writing-mode` `vertical`/`rl`/`sideways` with the given direction.
    #[cfg(test)]
    pub(crate) fn new(vertical: bool, rl: bool, sideways: bool, rtl: bool) -> Self {
        Self {
            vertical,
            rl: vertical && rl,
            sideways: vertical && sideways,
            turned: false,
            rtl,
            orientation: TextOrientation::Mixed,
        }
    }

    pub(crate) fn of(style: &Computed) -> Self {
        Self {
            turned: style.turned_line,
            ..WritingProps::of(style).wm()
        }
    }

    /// Laid out vertically in the current coordinate space: a vertical mode, except inside a
    /// laid-flat clone where the box did not declare its own mode.
    pub fn vertical(self) -> bool {
        self.vertical
    }

    /// The writing mode's block flow goes right-to-left (`vertical-rl`, `sideways-rl`).
    pub fn block_rl(self) -> bool {
        self.rl
    }

    /// `sideways-rl` / `sideways-lr`.
    pub(crate) fn sideways(self) -> bool {
        self.sideways
    }

    /// The box sits in a turned vertical line (rotation model).
    pub(crate) fn turned(self) -> bool {
        self.turned
    }

    /// `sideways-lr` (also laid flat): the one mode whose line runs bottom-to-top (§3.1).
    pub(crate) fn sideways_lr(self) -> bool {
        self.sideways && !self.rl
    }

    /// `direction: rtl`.
    pub fn rtl(self) -> bool {
        self.rtl
    }

    /// `text-orientation: upright`.
    pub(crate) fn upright(self) -> bool {
        self.orientation == TextOrientation::Upright
    }

    /// `text-orientation: sideways`: the alphabetic baseline is dominant (§4.2).
    pub(crate) fn text_sideways(self) -> bool {
        self.orientation == TextOrientation::Sideways
    }

    /// §4.2: the alphabetic baseline is dominant in sideways modes and with
    /// `text-orientation: sideways`; otherwise vertical text uses the central one.
    pub(crate) fn alphabetic_baseline(self) -> bool {
        self.sideways || self.text_sideways()
    }

    /// The writing mode of the LINE the box sits in: a turned line is vertical even though
    /// its clone is laid flat.
    pub(crate) fn line(self) -> Self {
        Self {
            vertical: self.vertical || self.turned,
            ..self
        }
    }

    /// Writing modes are orthogonal when their block axes are perpendicular
    /// (§7.1; Blink `IsParallelWritingMode`).
    pub(crate) fn is_orthogonal_to(self, other: Self) -> bool {
        self.vertical != other.vertical
    }

    /// The rotation of the turned-paragraph pipeline (§6.4 line-left / line-over).
    pub(crate) fn line_turn(self) -> Turn {
        match (self.vertical, self.sideways_lr()) {
            (false, _) => Turn::None,
            (true, false) => Turn::Clockwise,
            (true, true) => Turn::CounterClockwise,
        }
    }

    /// After a clockwise turn the first horizontal line becomes the RIGHT column; for
    /// `vertical-lr` (block-start = left) the lines are fed bottom-up so the first one lands
    /// on the left. `sideways-lr` turns counter-clockwise and needs no reversal.
    pub(crate) fn lines_reversed(self) -> bool {
        self.line_turn() == Turn::Clockwise && !self.rl
    }

    /// §6.4: line-left is the left edge in horizontal modes, the top after a clockwise
    /// turn and the bottom after a counter-clockwise one.
    pub(crate) fn line_left(self) -> Side {
        match self.line_turn() {
            Turn::None => Side::Left,
            Turn::Clockwise => Side::Top,
            Turn::CounterClockwise => Side::Bottom,
        }
    }

    /// Inline-start is line-left for `ltr` and line-right for `rtl` (§2.1, §6.4).
    pub(crate) fn inline_start(self) -> Side {
        let left = self.line_left();
        if self.rtl { left.opposite() } else { left }
    }

    /// §5.1: `text-orientation: upright` makes the used `direction` ltr in vertical
    /// typographic mode; `sideways-*` is horizontal typographic mode, where it does nothing.
    pub(crate) fn used_direction(self) -> Self {
        Self {
            rtl: self.rtl && !(self.vertical && !self.sideways && self.upright()),
            ..self
        }
    }

    /// Physical axes `[x, y]` that start at their far edge (right, bottom) — the scroll
    /// origin and grid flow (§6.4 with the used direction).
    pub(crate) fn reversed_axes(self) -> [bool; 2] {
        let wm = self.used_direction();
        let starts = [wm.block_start(), wm.inline_start()];
        [
            starts.contains(&Side::Right),
            starts.contains(&Side::Bottom),
        ]
    }

    /// Block-start (§6.4): top in horizontal modes, right for `*-rl`, left for `*-lr`.
    pub(crate) fn block_start(self) -> Side {
        match (self.vertical, self.rl) {
            (false, _) => Side::Top,
            (true, true) => Side::Right,
            (true, false) => Side::Left,
        }
    }
}

impl Computed {
    /// The one way to read this style's writing mode and direction.
    pub fn wm(&self) -> WritingMode {
        WritingMode::of(self)
    }

    /// The `writing-mode` this style typesets with, declared or laid flat (`None`: not
    /// declared anywhere up the merge chain).
    pub(crate) fn flow_mode(&self) -> Option<WritingModeKw> {
        self.writing_mode.or(self.flat_mode)
    }

    /// Whether a `writing-mode` applies to this style and is vertical (`None`: none declared
    /// on the own style, or up the merge chain of a merged one).
    pub(crate) fn vertical_decl(&self) -> Option<bool> {
        self.writing_mode.map(WritingModeKw::is_vertical)
    }

    /// Like `vertical_decl`, for the block flow of the mode typeset (laid flat included).
    pub(crate) fn rl_decl(&self) -> Option<bool> {
        self.flow_mode().map(WritingModeKw::is_rl)
    }

    /// Like `rl_decl`, for `sideways-*`.
    pub(crate) fn sideways_decl(&self) -> Option<bool> {
        self.flow_mode().map(WritingModeKw::is_sideways)
    }
}

#[cfg(test)]
mod tests;
