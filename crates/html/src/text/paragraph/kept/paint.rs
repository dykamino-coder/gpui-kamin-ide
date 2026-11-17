//! Paint of the paragraph's atoms, each with its own line (`super`).

use super::*;
use crate::layout::fragment::line_admission;
use gpui::{App, Window};

impl Paragraph {
    /// Paint the atoms (each with the line that holds its place, as in
    /// `atom_placement`): a resumed paragraph paints the atoms it built and
    /// offers the boxes of the others as ink.
    pub(in crate::text::paragraph) fn paint_atoms(
        &mut self,
        bands: &[(Pixels, Pixels)],
        window: &mut Window,
        cx: &mut App,
    ) {
        let lines = &self.lines;
        let row = |at: usize| lines.iter().position(|l| at < l.range.end);
        let paint =
            |slot: &mut AtomSlot, row: Option<usize>, window: &mut Window, cx: &mut App| match row
                .and_then(|row| bands.get(row))
            {
                Some(&band) => line_admission::paint(window, band, |w| slot.el.paint(w, cx)),
                None => slot.el.paint(window, cx),
            };
        if let Some(s) = self.kept.resumed() {
            let mut cursor = 0;
            for (_, slot) in s.built.iter_mut() {
                // Built atoms come in text order: their rows only grow.
                cursor += lines[cursor..]
                    .iter()
                    .position(|l| slot.at < l.range.end)
                    .unwrap_or(lines.len() - cursor);
                paint(slot, (cursor < lines.len()).then_some(cursor), window, cx);
            }
            if let Some(ink) = s.skipped {
                window.offer_ink(ink);
            }
            return;
        }
        for (k, slot) in self.atoms.iter_mut().enumerate().filter(|(_, s)| !s.hidden) {
            let at = self.atom_boxes.get(k).map(|b| b.at);
            paint(slot, at.and_then(row), window, cx);
        }
    }
}
