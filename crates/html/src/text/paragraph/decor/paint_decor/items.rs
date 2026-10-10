//! Items for paint_decor; split out to keep the owning module within 250 lines.

use super::DecorLine;
use super::*;
use crate::style::computed::{
    DECOR_OVER, DECOR_THROUGH, DECOR_UNDER, DecorLen, DecorStyle, UPOS_FROM_FONT, UPOS_UNDER,
};
use gpui::{Pixels, Window, px};

impl Paragraph {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_decor_items(
        &self,
        span: &DecorSpan,
        through: bool,
        s: usize,
        e: usize,
        line: &std::ops::Range<usize>,
        fx0: f32,
        fx1: f32,
        scale: f32,
        baseline: Pixels,
        dy: Pixels,
        segs: &mut Option<Vec<Seg>>,
        window: &mut Window,
        x_of: &dyn Fn(usize, usize) -> (Pixels, Pixels),
    ) {
        for item in &span.items {
            let d = &item.decor;
            let lines = if through {
                d.lines & DECOR_THROUGH
            } else {
                d.lines & (DECOR_UNDER | DECOR_OVER)
            };
            if lines == 0 {
                continue;
            }
            let ts = window.text_system().clone();
            let id = ts.resolve_font(&item.font);
            let size = px(d.font.size.max(0.01));
            let asc_f = f32::from(ts.ascent(id, size)) * scale;
            let desc_f = f32::from(ts.descent(id, size)).abs() * scale;
            let size_dev = d.font.size * scale;
            // css-text-decor-4 §2.4; Blink `ComputeDecorationThickness`:
            // `auto` is a tenth of the font size, a length is rounded.
            let auto_t = size_dev / 10.0;
            let t = match d.thickness {
                DecorLen::FromFont => {
                    let u = f32::from(ts.underline_thickness(id, size)) * scale;
                    if u > 0.0 { u } else { auto_t }
                }
                DecorLen::Px(v) => (v * scale).round(),
                _ => auto_t,
            }
            .max(1.0);
            let (l, r) = self.decor_insets(item, s, e, line, fx0, fx1, scale, t, segs, window);
            let x0 = (fx0 + l).round();
            let x1 = (fx1 - r).round();
            if x1 <= x0 {
                continue;
            }
            let color = d.color.to_hsla();
            let base_f = f32::from(baseline) * scale;
            let target_asc = self.run_ascent_at(s).map_or(asc_f, |a| a * scale);
            let text_top = base_f - target_asc;
            // (top of the line rect, second line of `double`, wave shift,
            // ink skipping) for each line of this decoration.
            let mut rows: Vec<(f32, f32, f32, bool)> = Vec::new();
            let off = match d.offset {
                DecorLen::Px(v) => Some(v * scale),
                _ => None,
            };
            // Em box edges below/above the baseline (Blink
            // `VerticalPosition(Bottom/TopOfEmHeight)`).
            let em_desc = if asc_f + desc_f > 0.0 {
                size_dev * desc_f / (asc_f + desc_f)
            } else {
                0.0
            };
            let em_asc = size_dev - em_desc;
            // Vertical text (painted in the rotated frame): Blink
            // `ResolveUnderlinePosition` — no alphabetic baseline; the
            // underline goes under the em box (line-left), or over it
            // (line-right) with `right`, swapping with the overline.
            let vertical = self.selection_vertical.is_some() && self.vertical_central_baseline;
            let flip = vertical
                && if d.over_lang {
                    d.position & crate::style::computed::UPOS_LEFT == 0
                } else {
                    d.position & crate::style::computed::UPOS_RIGHT != 0
                };
            // The underline offset is from the decorating box, on the
            // shared baseline (a shifted descendant keeps its box's line).
            let shift = (f32::from(self.group_shift(&item.group) - dy)) * scale;
            let base_u = base_f + shift;
            // Blink `ComputeUnderlineOffsetForUnder`: under the em box,
            // plus one pixel; over it, minus one pixel and the thickness.
            let under_em = |o: f32| base_u + em_desc + o + 1.0;
            let over_em = |o: f32| base_u - em_asc - o - 1.0 - t.floor();
            let under_at = |line: bool| -> (f32, f32, f32, bool) {
                (
                    under_em(if line { off.unwrap_or(0.0) } else { 0.0 }),
                    t + 1.0,
                    t + 1.0,
                    true,
                )
            };
            if lines & DECOR_UNDER != 0 {
                if flip {
                    rows.push((over_em(off.unwrap_or(0.0)), -(t + 1.0), -(t + 1.0), true));
                } else if d.position & UPOS_UNDER != 0 || vertical {
                    rows.push(under_at(true));
                } else {
                    let upos = f32::from(ts.underline_position(id, size)) * scale;
                    let y = if d.position & UPOS_FROM_FONT != 0 && upos != 0.0 {
                        base_u - upos + off.unwrap_or(0.0)
                    } else {
                        // Blink `ComputeUnderlineOffsetAuto`: a gap of half
                        // the thickness unless the offset is a length.
                        let gap = if off.is_none() {
                            (t / 2.0).ceil().max(1.0)
                        } else {
                            0.0
                        };
                        base_u + gap + off.unwrap_or(0.0)
                    };
                    rows.push((y, t + 1.0, t + 1.0, true));
                }
            }
            if lines & DECOR_OVER != 0 {
                if flip {
                    rows.push(under_at(false));
                } else {
                    // Blink `ComputeOverlineLineData`: grows up from the
                    // text top.
                    rows.push((text_top - t.floor(), -(t + 1.0), -(t + 1.0), true));
                }
            }
            if lines & DECOR_THROUGH != 0 {
                // Blink `ComputeLineThroughLineData`: centred at two
                // thirds of the ascent; ink is never skipped.
                rows.push((
                    text_top + 2.0 * asc_f / 3.0 - t / 2.0,
                    (t + 1.0).floor(),
                    0.0,
                    false,
                ));
            }
            for (y, double_offset, wavy_offset, ink) in rows {
                let mut parts = vec![(x0, x1)];
                if ink && span.skip_ink {
                    let row = (y + 0.5).floor();
                    let band = (row + 0.5, row + t.floor().max(1.0) - 0.5);
                    let dilation = t.min(13.0);
                    let base_g = base_f.round();
                    for (gx0, gx1, top, bottom) in self.ink_boxes(s, e, x_of, window) {
                        if base_g + bottom <= band.0 || base_g + top >= band.1 {
                            continue;
                        }
                        let (c0, c1) = ((gx0 - dilation).floor(), (gx1 + dilation).ceil());
                        parts = parts
                            .into_iter()
                            .flat_map(|(p0, p1)| {
                                let mut v = Vec::new();
                                if c0 > p0 {
                                    v.push((p0, c0.min(p1)));
                                }
                                if c1 < p1 {
                                    v.push((c1.max(p0), p1));
                                }
                                v
                            })
                            .filter(|(p0, p1)| p1 > p0)
                            .collect();
                    }
                }
                for (p0, p1) in parts {
                    let line = DecorLine {
                        style: d.style,
                        x0: p0,
                        x1: p1,
                        y,
                        t,
                        double_offset,
                        wavy_offset,
                        color,
                    };
                    if d.style == DecorStyle::Solid || d.style == DecorStyle::Double {
                        line.paint(scale, window);
                    } else {
                        // Patterned lines are merged across fragments so
                        // the pattern runs on (`text-decoration-dotted-001`).
                        let mut pending = self.decor_pending.borrow_mut();
                        match pending.last_mut() {
                            Some(last) if last.joins(&line) => last.x1 = line.x1,
                            _ => pending.push(line),
                        }
                    }
                }
            }
        }
    }
}
