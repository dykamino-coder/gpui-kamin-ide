//! Строчная сборка: адресованные абзацу измерения, trim и режим атомов.

use std::rc::Rc;

#[derive(Clone, Default)]
pub(crate) struct InlineContext {
    /// Внешний маркер выравнивает строки к пункту, независимо от CSS text-align.
    pub(crate) outside_marker_rtl: Option<bool>,
    /// Build a physical row of atoms without turning its enclosing text line.
    pub(crate) pure_atom_paragraph: bool,
    /// Intrinsic inline-size measurement offered to this build only.
    pub(crate) inline_measure: Option<crate::text::vertical::InlineMeasure>,
    /// `text-box-trim` requests of the trimming containers above, addressed
    /// to the blocks holding their first/last formatted line
    /// (`text::line_trim`).
    pub(crate) trim_requests: Rc<Vec<crate::text::line_trim::TrimRequest>>,
    /// The block whose paragraph is being built and its trim flags.
    pub(crate) trim_para: Option<(u64, crate::text::line_trim::Trim)>,
}
