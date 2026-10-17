//! One logical↔physical mapping for writing modes (css-writing-modes-4 §6.4 Abstract-Physical Mapping).
//!
//! Layout here is physical (GPUI/taffy boxes), so unlike Blink's `WritingModeConverter`
//! (`core/layout/geometry/writing_mode_converter.h`) or Servo's `LogicalVec2/LogicalSides`
//! (`components/layout/geom.rs`) this does not convert offsets: it answers how a flow-relative
//! direction lands physically — whether two boxes are orthogonal and how the turned vertical
//! paragraph is rotated. Paths that need that answer ask here instead of re-deriving it from the
//! `vertical`/`vertical_rl`/`sideways` flags; the queries grow with each migrated path
//! (`agents/cwmmap/design.md` stages).
use super::Computed;

/// How a horizontally typeset line is turned to become the vertical line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Turn {
    None,
    /// `vertical-*`, `sideways-rl`: line-left = top, line-over = right.
    Clockwise,
    /// `sideways-lr`: line-left = bottom, line-over = left.
    CounterClockwise,
}

/// The used writing mode of a box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct WritingMode {
    pub vertical: bool,
    /// Block flow right-to-left (`vertical-rl`, `sideways-rl`).
    pub rl: bool,
    pub sideways: bool,
}

impl WritingMode {
    pub(crate) fn of(style: &Computed) -> Self {
        let vertical = style.vertical == Some(true);
        Self {
            vertical,
            rl: vertical && style.vertical_rl == Some(true),
            sideways: vertical && style.sideways == Some(true),
        }
    }

    /// The writing mode of the box's containing block, recorded at inheritance
    /// (`cascade/inherit/containing.rs`).
    pub(crate) fn containing_block(style: &Computed) -> Self {
        Self {
            vertical: style.cb_vertical,
            rl: style.cb_vertical && style.cb_vertical_rl,
            sideways: style.cb_vertical && style.cb_sideways,
        }
    }

    /// Writing modes are orthogonal when their block axes are perpendicular
    /// (§7.1; Blink `IsParallelWritingMode`).
    pub(crate) fn is_orthogonal_to(self, other: Self) -> bool {
        self.vertical != other.vertical
    }

    /// The rotation of the turned-paragraph pipeline (§6.4 line-left / line-over).
    pub(crate) fn line_turn(self) -> Turn {
        match (self.vertical, self.sideways && !self.rl) {
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
}

impl Computed {
    pub(crate) fn wm(&self) -> WritingMode {
        WritingMode::of(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(vertical: bool, rl: bool, sideways: bool) -> WritingMode {
        WritingMode {
            vertical,
            rl,
            sideways,
        }
    }

    #[test]
    fn turn_and_line_order_follow_the_abstract_physical_mapping() {
        // (mode, turn, lines fed bottom-up)
        let rows = [
            (mode(false, false, false), Turn::None, false),
            (mode(true, true, false), Turn::Clockwise, false),
            (mode(true, false, false), Turn::Clockwise, true),
            (mode(true, true, true), Turn::Clockwise, false),
            (mode(true, false, true), Turn::CounterClockwise, false),
        ];
        for (m, turn, reversed) in rows {
            assert_eq!(m.line_turn(), turn, "{m:?}");
            assert_eq!(m.lines_reversed(), reversed, "{m:?}");
        }
    }

    #[test]
    fn flags_are_read_only_under_a_vertical_mode() {
        let style = Computed {
            vertical_rl: Some(true),
            sideways: Some(true),
            ..Computed::default()
        };
        assert_eq!(style.wm(), WritingMode::default());
        let child = Computed {
            vertical: Some(true),
            cb_vertical: false,
            ..Computed::default()
        };
        assert!(
            child
                .wm()
                .is_orthogonal_to(WritingMode::containing_block(&child))
        );
    }
}
