//! Select native vertical shaping from actual collected pieces without rebuilding atoms.
use crate::{computed::Computed, inline::Piece};
use std::cell::Cell;

pub(super) struct Request<'a> {
    pub style: &'a Computed,
    pub built: &'a Cell<bool>,
}

impl Request<'_> {
    pub fn accepts(&self, pieces: &[Piece], has_line_atoms: bool) -> bool {
        // Inline atoms converted to text spacers still need their physical placement.
        // Out-of-flow overlays likewise retain the existing coordinate contract.
        !has_line_atoms && pieces.iter().all(|piece| matches!(piece, Piece::Text { .. }))
    }
}
