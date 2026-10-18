//! The writing-mode properties of a style as the cascade inherits them: `writing-mode`
//! (with the rotation model's laid-flat mode) and `direction`. Used by the cascade merge
//! (`cascade/inherit/fonts.rs`) and by the logical-property mapping pass
//! (`document/logical_properties.rs`), so both inherit by the same rule.
use super::{WritingMode, WritingModeKw};
use crate::style::computed::Computed;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct WritingProps {
    pub writing_mode: Option<WritingModeKw>,
    pub flat_mode: Option<WritingModeKw>,
    pub rtl: Option<bool>,
}

impl WritingProps {
    pub(crate) fn of(style: &Computed) -> Self {
        Self {
            writing_mode: style.writing_mode,
            flat_mode: style.flat_mode,
            rtl: style.rtl,
        }
    }

    /// CSS Cascade 4 §7.2: an undeclared inherited property takes the parent's value.
    pub(crate) fn inherit(self, parent: Self) -> Self {
        Self {
            writing_mode: self.writing_mode.or(parent.writing_mode),
            flat_mode: self.flat_mode.or(parent.flat_mode),
            rtl: self.rtl.or(parent.rtl),
        }
    }

    /// The value outside any turned line.
    pub(crate) fn wm(self) -> WritingMode {
        let flow = self.writing_mode.or(self.flat_mode).unwrap_or_default();
        WritingMode {
            vertical: self.writing_mode.is_some_and(WritingModeKw::is_vertical),
            rl: flow.is_rl(),
            sideways: flow.is_sideways(),
            turned: false,
            rtl: self.rtl == Some(true),
        }
    }
}
