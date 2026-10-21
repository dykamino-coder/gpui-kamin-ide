//! Свойства режима письма в стиле так, как их наследует каскад: `writing-mode`
//! (с уложенным режимом модели поворота), `direction` и `text-orientation`. Используются слиянием
//! каскада (`cascade/inherit/fonts.rs`) и проходом логических свойств
//! (`document/logical_properties.rs`), поэтому оба наследуют по одному правилу.
use super::{TextOrientation, WritingMode, WritingModeKw};
use crate::style::computed::Computed;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct WritingProps {
    pub writing_mode: Option<WritingModeKw>,
    pub flat_mode: Option<WritingModeKw>,
    pub rtl: Option<bool>,
    pub text_orientation: Option<TextOrientation>,
}

impl WritingProps {
    pub(crate) fn of(style: &Computed) -> Self {
        Self {
            writing_mode: style.writing_mode,
            flat_mode: style.flat_mode,
            rtl: style.rtl,
            text_orientation: style.text_orientation,
        }
    }

    /// CSS Cascade 4 §7.2: an undeclared inherited property takes the parent's value.
    pub(crate) fn inherit(self, parent: Self) -> Self {
        Self {
            writing_mode: self.writing_mode.or(parent.writing_mode),
            flat_mode: self.flat_mode.or(parent.flat_mode),
            rtl: self.rtl.or(parent.rtl),
            text_orientation: self.text_orientation.or(parent.text_orientation),
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
            orientation: self.text_orientation.unwrap_or_default(),
        }
    }
}
