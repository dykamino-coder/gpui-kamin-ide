//! Resolve gradient font units after the element's font and line height are known.

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::values::value::{Len, root_font_px};

impl Computed {
    pub(crate) fn resolve_image_lengths(&mut self, vcx: ValueCx<'_>) {
        // Font probes belong only to images with relative lengths. Besides
        // avoiding needless shaping, this keeps paint-time font measurement
        // out of ordinary backgrounds and masks.
        let position_units = self
            .bg_position_math
            .iter()
            .flatten()
            .any(|value| value.needs_font_resolution())
            || self
                .bg_lists
                .position_expressions()
                .any(|value| value.needs_font_resolution());
        if !position_units {
            return;
        }
        let (font, ex, ch, line) = self.image_font_metrics(vcx);
        // Relative units become computed lengths before position inheritance;
        // only the percentage basis must remain for concrete image geometry.
        for expression in self
            .bg_position_math
            .iter_mut()
            .flatten()
            .chain(self.bg_lists.position_expressions())
        {
            expression.resolve_font(font, root_font_px(vcx), ex, ch, line, vcx);
        }
    }

    pub(in crate::style::computed) fn image_font_metrics(
        &self,
        vcx: ValueCx<'_>,
    ) -> (f32, f32, f32, f32) {
        let font = match self.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let family = self.font_family.as_deref().unwrap_or("");
        let (ch, ex) = crate::text::metrics::ch_ex_px(family, font, vcx);
        let line = match self.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(k)) | Some(Len::Em(k)) => k * font,
            _ => {
                let (ascent, descent, _) = crate::text::metrics::vmetrics_px(family, font, vcx);
                ascent + descent
            }
        };
        (font, ex, ch, line)
    }
}

pub(in crate::style::computed) fn lengths(
    raw: &str,
    em: f32,
    rem: f32,
    ex: f32,
    ch: f32,
    lh: f32,
) -> String {
    let raw = crate::style::computed::font_lengths_to_px(raw, em, rem, ex, ch);
    let mut output = String::with_capacity(raw.len());
    let mut token = String::new();
    let flush = |token: &mut String, output: &mut String| {
        if let Some(number) = token.strip_suffix("lh").and_then(|n| n.parse::<f32>().ok()) {
            output.push_str(&format!("{}px", number * lh));
        } else {
            output.push_str(token);
        }
        token.clear();
    };
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
            token.push(c);
        } else {
            flush(&mut token, &mut output);
            output.push(c);
        }
    }
    flush(&mut token, &mut output);
    output
}
