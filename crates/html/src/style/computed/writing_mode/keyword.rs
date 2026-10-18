//! The `writing-mode` (css-writing-modes-4 §3.1) and `text-orientation` (§5.1) keywords ( Blink `WritingMode` enum,
//! `platform/text/writing_mode.h`): the stored value of the property.

/// A `writing-mode` value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum WritingModeKw {
    #[default]
    HorizontalTb,
    VerticalRl,
    VerticalLr,
    SidewaysRl,
    SidewaysLr,
}

impl WritingModeKw {
    /// The keyword with a vertical block flow (`rl`) and glyph orientation (`sideways`);
    /// `vertical == false` is `horizontal-tb` whatever the other two say.
    pub(crate) fn new(vertical: bool, rl: bool, sideways: bool) -> Self {
        match (vertical, rl, sideways) {
            (false, _, _) => Self::HorizontalTb,
            (true, true, false) => Self::VerticalRl,
            (true, false, false) => Self::VerticalLr,
            (true, true, true) => Self::SidewaysRl,
            (true, false, true) => Self::SidewaysLr,
        }
    }

    /// The declared keyword. Unknown values count as `horizontal-tb`, as the
    /// property parser always did.
    pub(crate) fn parse(value: &str) -> Self {
        let vertical = value.starts_with("vertical") || value.starts_with("sideways");
        Self::new(
            vertical,
            value.ends_with("rl"),
            value.starts_with("sideways"),
        )
    }

    pub(crate) fn is_vertical(self) -> bool {
        self != Self::HorizontalTb
    }

    /// Block flow right-to-left.
    pub(crate) fn is_rl(self) -> bool {
        matches!(self, Self::VerticalRl | Self::SidewaysRl)
    }

    pub(crate) fn is_sideways(self) -> bool {
        matches!(self, Self::SidewaysRl | Self::SidewaysLr)
    }
}

/// A `text-orientation` value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum TextOrientation {
    #[default]
    Mixed,
    Upright,
    /// `sideways` and its legacy alias `sideways-right`.
    Sideways,
}

impl TextOrientation {
    /// Unknown values count as `mixed`, as the property parser always did.
    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "upright" => Self::Upright,
            "sideways" | "sideways-right" => Self::Sideways,
            _ => Self::Mixed,
        }
    }
}
