//! Physical text layout inside one inline formatting context (cascade::inherit::context
//! puts it on the line's contents): the element's computed axes and font units are kept.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InlineFormat {
    Turned,
    Upright,
    Combined,
}

impl BoxStyle {
    /// The box sits in a turned vertical line (rotation model): inside the
    /// horizontal clone of a turned vertical paragraph.
    pub fn turned_line(&self) -> bool {
        self.tree.turned_line
    }

    /// The writing mode of the line the box sits in (`WritingMode::on_line`).
    pub fn line_wm(&self) -> super::writing_mode::WritingMode {
        self.wm().on_line(self.tree.turned_line)
    }
}
