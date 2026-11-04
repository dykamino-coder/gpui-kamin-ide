//! Layout key of a paragraph's measured leaf (`gpui::Window::set_next_layout_key`).
//!
//! The window's frame tree keeps a measured leaf across frames when the new
//! request has the key of a previous one, and with the leaf its layout cache:
//! the line splits of an unchanged paragraph are not measured again on a
//! resize whose constraints reach it unchanged. The key therefore covers
//! every input of the measure probe (`request_layout_impl`). A paragraph whose
//! probe also reads state shared with other elements or bound later (atoms,
//! overlays, ruby, fragments, clamps, fitting, vertical or rotated text) has
//! no key and is measured in every frame, as before.

use crate::text::paragraph::Paragraph;
use std::hash::{Hash, Hasher};

/// Formatter sink that feeds the hasher without building a string.
struct Sink<'a>(&'a mut std::collections::hash_map::DefaultHasher);

impl std::fmt::Write for Sink<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

impl Paragraph {
    /// Key of everything the measure probe reads, or `None` when the probe
    /// reads state a key cannot see. `rebuild` is a probe rebuilt per call.
    pub(super) fn layout_key(&self, rebuild: bool) -> Option<u64> {
        if !self.keyable(rebuild) {
            return None;
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.text.as_ref().hash(&mut hasher);
        // `Debug` of these values is exact (floats print round-trip).
        std::fmt::write(
            &mut Sink(&mut hasher),
            format_args!(
                "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
                self.runs,
                self.font_size,
                self.line_height,
                self.align,
                self.wrap,
                self.spans,
                self.word_spans,
                self.letter_spans,
                self.shift_spans,
                self.lh_spans,
                self.clamp_force,
                self.clamp_tag,
                self.tab_stop,
                self.indent,
                self.hanging,
                self.flow,
                self.first_strut,
                self.run_metrics,
                (self.strut, self.strut_box),
                (self.lines_reversed, self.ruby_trim),
            ),
        )
        .ok()?;
        Some(hasher.finish())
    }

    /// Whether the probe reads only the paragraph's own values.
    fn keyable(&self, rebuild: bool) -> bool {
        !rebuild
            && self.overlays.is_empty()
            && self.atoms.is_empty()
            && self.atom_boxes.is_empty()
            && self.ruby_base_sink.is_none()
            && self.ruby_layers.is_none()
            && !self.ruby_unit
            && !self.ruby_justify
            && self.fragment.is_none()
            && self.clamp_rows.is_none()
            && self.clamp.is_none()
            && self.fit.is_none()
            && self.inline_measure.is_none()
            && !self.vertical
            && self.vertical_inline.is_none()
            && self.ortho_limit.is_none()
            && !self.paint_context.is_rotated()
            && self.spacers.is_empty()
            && self.spacer_edges.is_empty()
            && self.box_extents.is_empty()
            && self.emph_spans.is_empty()
            && self.box_spans.is_empty()
            && self.edge_spans.is_empty()
    }
}
