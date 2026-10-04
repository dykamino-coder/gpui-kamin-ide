//! Single-font measurement for layout construction before a window is available.

use super::{Font, FontRun, TextSystem};
use crate::Pixels;

impl TextSystem {
    /// Shape a line using the platform text system and return its advance.
    ///
    /// Unlike summing glyph advances, shaping applies font features and glyph
    /// fallback. This is a measurement only; decoration and painting use the
    /// window text system as before.
    pub fn measure_line(&self, text: &str, font: &Font, size: Pixels) -> Pixels {
        let run = FontRun {
            len: text.len(),
            font_id: self.resolve_font(font),
            font_size: size,
        };
        self.platform_text_system
            .layout_line(text, size, &[run])
            .width
    }
}
