//! OpenType MATH glyph selection; no geometric scaling of glyph outlines.
use super::{assembly, geometry::Metrics};
use crate::text::fonts::math;
use gpui::{Font, FontId, GlyphId, Hsla, Pixels, Point, Window, point, px};
use ttf_parser::{Face, GlyphId as TtfGlyph};

pub(super) struct Plan {
    pub metrics: Metrics,
    pub parts: Vec<(u16, f32, f32)>,
    font: FontId,
    size: f32,
}
impl Plan {
    pub fn new(
        font: &Font,
        size: f32,
        ch: char,
        target: f32,
        vertical: bool,
        window: &Window,
        vcx: crate::document::state::ValueCx<'_>,
    ) -> Option<Self> {
        let Some(bytes) = math::font_bytes(&font.family, vcx) else {
            return Self::fallback(font, size, ch, window);
        };
        let face = Face::parse(&bytes, 0).ok()?;
        let base = face.glyph_index(ch)?;
        let scale = size / f32::from(face.units_per_em());
        let variants = face.tables().math.and_then(|m| m.variants);
        let construction = variants.and_then(|v| {
            if vertical {
                v.vertical_constructions.get(base)
            } else {
                v.horizontal_constructions.get(base)
            }
        });
        let mut selected = base;
        if let Some(construction) = construction {
            for variant in construction.variants {
                selected = variant.variant_glyph;
                if f32::from(variant.advance_measurement) * scale >= target {
                    break;
                }
            }
            let natural = bounds(&face, selected, scale)?;
            let advance = if vertical {
                natural.ascent + natural.descent
            } else {
                natural.width
            };
            if advance < target
                && let Some(assembly) = construction.assembly
            {
                let parts = assembly::parts(
                    assembly,
                    variants?.min_connector_overlap,
                    target / scale,
                    vertical,
                );
                if !parts.is_empty() {
                    return Self::from_parts(&face, font, size, scale, parts, window);
                }
            }
        }
        Self::from_parts(
            &face,
            font,
            size,
            scale,
            vec![(selected.0, 0.0, 0.0)],
            window,
        )
    }
    fn fallback(font: &Font, size: f32, ch: char, window: &Window) -> Option<Self> {
        let text = ch.to_string();
        let run = gpui::TextRun {
            len: text.len(),
            font: font.clone(),
            ..Default::default()
        };
        let line = window
            .text_system()
            .shape_line(text.into(), px(size), &[run], None);
        let run = line.runs.first()?;
        let glyph = run.glyphs.first()?;
        let bounds = window
            .text_system()
            .typographic_bounds(run.font_id, px(size), ch)
            .ok()?;
        Some(Self {
            metrics: Metrics {
                width: line.width.into(),
                ascent: f32::from(bounds.origin.y + bounds.size.height),
                descent: -f32::from(bounds.origin.y),
            },
            parts: vec![(u16::try_from(glyph.id.0).ok()?, 0.0, 0.0)],
            font: run.font_id,
            size,
        })
    }
    fn from_parts(
        face: &Face<'_>,
        font: &Font,
        size: f32,
        scale: f32,
        mut parts: Vec<(u16, f32, f32)>,
        window: &Window,
    ) -> Option<Self> {
        let (mut top, mut bottom, mut width) = (0.0f32, 0.0f32, 0.0f32);
        for (id, x, y) in &mut parts {
            *x *= scale;
            *y *= scale;
            let b = bounds(face, TtfGlyph(*id), scale)?;
            top = top.max(b.ascent - *y);
            bottom = bottom.max(b.descent + *y);
            width = width.max(*x + b.width);
        }
        Some(Self {
            metrics: Metrics {
                width,
                ascent: top,
                descent: bottom,
            },
            parts,
            font: window.text_system().resolve_font(font),
            size,
        })
    }
    pub fn paint(&self, origin: Point<Pixels>, color: Hsla, window: &mut Window) {
        for &(id, x, y) in &self.parts {
            let _ = window.paint_glyph(
                origin + point(px(x), px(y + self.metrics.ascent)),
                self.font,
                GlyphId(u32::from(id)),
                px(self.size),
                color,
            );
        }
    }
}
fn bounds(face: &Face<'_>, id: TtfGlyph, scale: f32) -> Option<Metrics> {
    let b = face.glyph_bounding_box(id)?;
    Some(Metrics {
        width: f32::from(face.glyph_hor_advance(id)?) * scale,
        ascent: f32::from(b.y_max) * scale,
        descent: -f32::from(b.y_min) * scale,
    })
}
