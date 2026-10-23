//! Маршрутизация абзаца: сбор и подготовка строчных кусков (`collect`), выбор
//! пути по готовым кускам — своя раскладка строк, сплошной текст или ряд из слов
//! (`choose`), сборка `Paragraph` (`native`), а снаружи — проба бюджета строк и
//! шаг 7 окраски (`probed`).
//!
//! Запрос родного вертикального набора (`NativeRequest`) решается по уже
//! собранным кускам, без пересборки атомов. Он принимается только для абзаца из
//! одних текстовых кусков: строчные атомы, ставшие текстовыми заполнителями, и
//! внепоточные наложения сохраняют прежний контракт физических координат.

use crate::text::inline::Piece;
use gpui::AnyElement;
use std::cell::Cell;

mod choose;
mod collect;
mod native;
mod probed;
mod spans;
mod styling;

pub(crate) use collect::paragraph_pieces_routed;
pub(crate) use probed::{paint_inline_step7, paragraph_probed};

/// Атомы, поставленные в строку абзаца: байтовое место распорки в тексте,
/// элемент, выравнивание по строке и протяжённость руби.
pub(crate) type LineAtoms = Vec<(
    usize,
    AnyElement,
    crate::text::paragraph::AtomAlign,
    crate::text::paragraph::RubyExtents,
)>;

pub(crate) struct NativeRequest<'a> {
    pub style: &'a crate::style::computed::BoxStyle,
    pub built: &'a Cell<bool>,
}

impl NativeRequest<'_> {
    pub fn accepts(&self, pieces: &[Piece], has_line_atoms: bool) -> bool {
        // Inline atoms converted to text spacers still need their physical placement.
        // Out-of-flow overlays likewise retain the existing coordinate contract.
        !has_line_atoms
            && pieces
                .iter()
                .all(|piece| matches!(piece, Piece::Text { .. }))
    }
}
