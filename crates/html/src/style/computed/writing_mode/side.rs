//! Физические стороны и повороты строк, в которых отвечает отображение abstract-physical (§6.4).

/// How a horizontally typeset line is turned to become the vertical line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Turn {
    None,
    /// `vertical-*`, `sideways-rl`: line-left = top, line-over = right.
    Clockwise,
    /// `sideways-lr`: line-left = bottom, line-over = left.
    CounterClockwise,
}

/// A physical box side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

impl Side {
    /// Index in physical `[top, right, bottom, left]` arrays.
    pub(crate) fn index(self) -> usize {
        match self {
            Side::Top => 0,
            Side::Right => 1,
            Side::Bottom => 2,
            Side::Left => 3,
        }
    }

    pub(crate) fn opposite(self) -> Self {
        match self {
            Side::Top => Side::Bottom,
            Side::Bottom => Side::Top,
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}
