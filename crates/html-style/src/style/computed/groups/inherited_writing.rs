//! CSS-операнды группы writing; значение за общим COW-указателем.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InheritedWriting {
    /// `direction: rtl` — письмо справа налево.
    pub rtl: Option<bool>,
    /// `text-combine-upright`: сколько знаков сжимается в один кегль
    /// (0 — `all`, 2..=4 — `digits N`; `Some(None)` — `none`). Наследуется.
    pub combine_upright: Option<Option<u8>>,
    /// `writing-mode` (css-writing-modes-4 §3.1): declared on the own style, inherited in
    /// the merged one; `None` = not declared. Read through `Computed::wm()`.
    pub writing_mode: Option<crate::style::computed::writing_mode::WritingModeKw>,
    /// Rotation model: the vertical mode whose lines this horizontal clone typesets laid
    /// flat (turned paragraph, upright stack). Inherited; read through `Computed::wm()`.
    pub flat_mode: Option<crate::style::computed::writing_mode::WritingModeKw>,
    /// `text-orientation` (css-writing-modes-4 §5.1); `None` = not declared. `upright` also
    /// changes `ch` (the zero advances along the vertical line), `sideways` makes the
    /// alphabetic baseline dominant (§4.2). Read through `Computed::wm()`.
    pub text_orientation: Option<crate::style::computed::writing_mode::TextOrientation>,
}
