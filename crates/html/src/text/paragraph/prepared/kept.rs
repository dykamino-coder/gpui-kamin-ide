//! Break opportunities carried over to a probe made later from the same text
//! and rules (`element::layout::plain_probe`).

use super::super::Paragraph;

impl Paragraph {
    /// Seed the break opportunities of the same text and rules.
    pub(in crate::text::paragraph) fn seed_stops(&self, stops: Box<[u32]>) {
        let _ = self.prepared().stops.set(stops);
    }
}
