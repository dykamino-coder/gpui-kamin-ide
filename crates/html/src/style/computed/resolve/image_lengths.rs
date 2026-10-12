//! Resolve gradient font units after the element's font and line height are known.

use crate::style::computed::Computed;
use crate::style::values::value::{Len, root_font_px};

impl Computed {
    pub(crate) fn resolve_image_lengths(&mut self) {
        // Font probes belong only to images with relative lengths. Besides
        // avoiding needless shaping, this keeps paint-time font measurement
        // out of ordinary backgrounds and masks.
        if ![&self.bg_image, &self.gradient_raw, &self.mask_image]
            .into_iter()
            .flatten()
            .any(|raw| {
                raw.contains("gradient(")
                    && (raw.contains("lh") || crate::style::computed::has_font_units(raw))
            })
        {
            return;
        }
        let font = match self.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let family = self.font_family.as_deref().unwrap_or("");
        let (ch, ex) = crate::text::metrics::ch_ex_px(family, font);
        let line = match self.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(k)) | Some(Len::Em(k)) => k * font,
            _ => {
                let (ascent, descent, _) = crate::text::metrics::vmetrics_px(family, font);
                ascent + descent
            }
        };
        for raw in [
            &mut self.bg_image,
            &mut self.gradient_raw,
            &mut self.mask_image,
        ]
        .into_iter()
        .flatten()
        {
            if !raw.contains("gradient(") {
                continue;
            }
            *raw = lengths(raw, font, root_font_px(), ex, ch, line);
        }
    }
}

fn lengths(raw: &str, em: f32, rem: f32, ex: f32, ch: f32, lh: f32) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_and_mask_share_computed_line_height() {
        let mut c = Computed {
            font_size: Some(Len::Px(50.0)),
            line_height: Some(Len::Pct(2.0)),
            bg_image: Some("conic-gradient(at 1lh 1em, red, green)".into()),
            mask_image: Some("conic-gradient(at 1lh 1em, red, green)".into()),
            ..Computed::default()
        };
        c.resolve_image_lengths();
        assert_eq!(c.bg_image, c.mask_image);
        assert!(c.bg_image.unwrap().contains("at 100px 50px"));
    }
}
