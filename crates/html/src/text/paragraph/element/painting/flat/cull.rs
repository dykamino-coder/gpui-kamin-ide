//! Lines of a horizontal paragraph that paint nothing visible are not painted
//! (Blink `BoxFragmentPainter::PaintLineBoxes`: a line whose ink overflow
//! misses the cull rect in the block direction is skipped).
//!
//! The scene drops every primitive outside the content mask, so a line whose
//! ink lies wholly outside it changes no pixel. A line's ink is bounded by its
//! band grown by the paragraph's bleed: the glyph boxes of every font the
//! shaped runs use (the `head` box, plus glyph offsets and synthetic emboldening),
//! vertical shifts and relative offsets of pieces, inline box padding and
//! borders, line paddings and run decorations drawn by the text system. A
//! paragraph whose painting has other inputs (its own decorations and
//! emphasis marks, ruby layers, fragment admission) is painted whole, and so
//! is a paragraph painted outside the window's root environment
//! (transforms, opacity, masks, groups, segment capture), where the mask is
//! not in window space. Open ink sinks (`gpui::InkSink`) still receive the
//! bounds of a skipped line: they measure culled ink too.

use crate::text::paragraph::*;
use gpui::{Bounds, Pixels, Window, point, px, size};

/// Vertical extent of the window content mask a paragraph's lines are culled
/// against, with the bleed of its ink beyond a line band.
pub(super) struct Cull {
    top: Pixels,
    bottom: Pixels,
    bleed: Pixels,
    /// Horizontal extent offered to ink sinks for a skipped line.
    left: Pixels,
    right: Pixels,
}

impl Cull {
    /// Culling for this paint, or `None` when every line is painted.
    pub(super) fn new(
        p: &Paragraph,
        segs: &[Seg],
        pads: &[(f32, f32)],
        bounds: Bounds<Pixels>,
        window: &mut Window,
    ) -> Option<Self> {
        if !window.retained_root_environment()
            || p.fragment.is_some()
            || p.ruby_layers.is_some()
            || !p.emph_spans.is_empty()
            || p.decor_on()
        {
            return None;
        }
        let mask = window.content_mask().bounds;
        let bleed = bleed(p, segs, pads, window);
        let widest = p
            .lines
            .iter()
            .map(|l| l.width + l.indent.abs())
            .fold(px(0.), Pixels::max);
        Some(Self {
            top: mask.origin.y,
            bottom: mask.origin.y + mask.size.height,
            bleed,
            left: bounds.origin.x - widest - bleed,
            right: bounds.origin.x + bounds.size.width + widest + bleed,
        })
    }

    /// Skip the line of band `band` (top, bottom): its ink cannot reach the
    /// mask. The skipped ink is offered to open ink sinks.
    pub(super) fn skip(&self, band: (Pixels, Pixels), window: &mut Window) -> bool {
        match self.ink_outside(band) {
            Some(ink) => {
                window.offer_ink(ink);
                true
            }
            None => false,
        }
    }

    /// The ink bounds of a line of band `band` when they miss the mask.
    fn ink_outside(&self, band: (Pixels, Pixels)) -> Option<Bounds<Pixels>> {
        let (top, bottom) = (band.0 - self.bleed, band.1 + self.bleed);
        (bottom <= self.top || top >= self.bottom).then(|| Bounds {
            origin: point(self.left, top),
            size: size(self.right - self.left, bottom - top),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_line_whose_grown_band_misses_the_mask_is_skipped() {
        let cull = Cull {
            top: px(100.),
            bottom: px(700.),
            bleed: px(30.),
            left: px(-50.),
            right: px(850.),
        };
        assert!(
            cull.ink_outside((px(20.), px(69.))).is_some(),
            "above, beyond the bleed"
        );
        assert!(
            cull.ink_outside((px(50.), px(71.))).is_none(),
            "its bleed reaches the mask"
        );
        assert!(
            cull.ink_outside((px(400.), px(420.))).is_none(),
            "on screen"
        );
        assert!(
            cull.ink_outside((px(731.), px(750.))).is_some(),
            "below, beyond the bleed"
        );
        assert!(
            cull.ink_outside((px(729.), px(750.))).is_none(),
            "its bleed reaches the mask"
        );
        let ink = cull.ink_outside((px(800.), px(820.))).expect("below");
        assert_eq!(ink.origin, point(px(-50.), px(770.)));
        assert_eq!(ink.size, size(px(900.), px(80.)));
    }
}

/// How far a line's ink may reach beyond its band in the block direction.
fn bleed(p: &Paragraph, segs: &[Seg], pads: &[(f32, f32)], window: &mut Window) -> Pixels {
    let mut glyph = p.font_size;
    let mut offset = px(0.);
    let text = window.text_system().clone();
    // Соседние прогоны чаще всего одного шрифта: повтор не спрашивается (без памяти на список).
    let mut last: Option<(gpui::FontId, Pixels)> = None;
    for seg in segs {
        for run in &seg.layout.runs {
            let size = if run.font_size > px(0.) {
                run.font_size
            } else {
                seg.layout.font_size
            };
            if last != Some((run.font_id, size)) {
                last = Some((run.font_id, size));
                let b = text.bounding_box(run.font_id, size);
                let reach = b.origin.y.abs().max((b.origin.y + b.size.height).abs());
                // Synthetic emboldening and oblique grow the outline by a
                // fraction of the size; antialiasing by a device pixel.
                glyph = glyph.max(reach + size / 8.);
            }
            for g in &run.glyphs {
                offset = offset.max(g.position.y.abs());
            }
        }
    }
    let mut boxes = px(0.);
    for run in &p.runs {
        let widest = |w: &[Pixels; 4]| w.iter().map(|v| f32::from(*v).abs()).fold(0., f32::max);
        let pad = widest(&run.background_pad);
        let border = run
            .background_border
            .as_ref()
            .map_or(0., |(_, w)| widest(w));
        let gap = if run.background_line_gap {
            f32::from(p.line_height)
        } else {
            0.
        };
        let lines = run.underline.map_or(0., |u| f32::from(u.thickness) * 4.)
            + run
                .strikethrough
                .map_or(0., |s| f32::from(s.thickness) * 4.);
        boxes = boxes.max(px(pad + border + gap + lines));
    }
    let shift = p
        .shift_spans
        .iter()
        .map(|(_, v)| v.abs())
        .fold(px(0.), Pixels::max);
    let rel = p
        .rel_spans
        .iter()
        .map(|(_, a, b)| a.1.abs().max(b.1.abs()))
        .fold(0., f32::max);
    let pads = pads
        .iter()
        .map(|(a, b)| a.abs() + b.abs())
        .fold(0., f32::max);
    glyph + offset + boxes + shift + px(rel + pads) + p.line_height + px(2.)
}
