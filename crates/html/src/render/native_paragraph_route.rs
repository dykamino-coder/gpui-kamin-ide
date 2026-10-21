//! Сборка абзаца: выбор родного вертикального набора по уже собранным кускам
//! (`Piece`), без пересборки атомов. Запрос принимается только для абзаца из
//! одних текстовых кусков: строчные атомы, ставшие текстовыми заполнителями, и
//! внепоточные наложения сохраняют прежний контракт физических координат.
use crate::style::computed::Computed;
use crate::text::inline::Piece;
use std::cell::Cell;

pub(crate) struct Request<'a> {
    pub style: &'a Computed,
    pub built: &'a Cell<bool>,
}

impl Request<'_> {
    pub fn accepts(&self, pieces: &[Piece], has_line_atoms: bool) -> bool {
        // Inline atoms converted to text spacers still need their physical placement.
        // Out-of-flow overlays likewise retain the existing coordinate contract.
        !has_line_atoms
            && pieces
                .iter()
                .all(|piece| matches!(piece, Piece::Text { .. }))
    }
}
