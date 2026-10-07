//! Text decoration lines (css-text-decor-3 §2, css-text-decor-4 §2–§4).
//!
//! Geometry follows Blink `TextDecorationInfo` / `TextDecorationOffset` /
//! `DecorationLinePainter`, which work in device pixels (zoom-for-DSF): the
//! thickness is rounded there, the line is snapped vertically to the device
//! grid, and the run ends land on whole device pixels.

use super::*;
use crate::computed::{
    DECOR_OVER, DECOR_THROUGH, DECOR_UNDER, DecorLen, DecorStyle, UPOS_FROM_FONT, UPOS_UNDER,
};

impl Paragraph {
    /// Lines are painted by the paragraph itself (not by GPUI's run
    /// underline) for horizontal text with decorated pieces.
    pub(super) fn decor_on(&self) -> bool {
        !self.decor_spans.is_empty()
    }

    /// Ascent (logical px) of the run that holds byte `at`.
    fn run_ascent_at(&self, at: usize) -> Option<f32> {
        if self.run_metrics.len() != self.runs.len() {
            return None;
        }
        let mut acc = 0usize;
        for (r, m) in self.runs.iter().zip(&self.run_metrics) {
            let st = acc;
            acc += r.len;
            if at >= st && at < acc {
                return Some(m.0);
            }
        }
        None
    }

    /// Decorations of one painted shaped run (`run` — bytes of the paragraph,
    /// `at` — the run's line-box origin, as given to `ShapedLine::paint`).
    pub(super) fn paint_decor_shaped(
        &self,
        run: &std::ops::Range<usize>,
        shaped: &gpui::ShapedLine,
        at: Point<Pixels>,
        dy: Pixels,
        rtl: bool,
        line: &std::ops::Range<usize>,
        through: bool,
        window: &mut Window,
    ) {
        let src = &self.text[run.clone()];
        if rtl {
            // Right-to-left run: the extent of `a..b` is the union of the
            // advances of its glyphs (visual order), whole run when covered.
            let lead = shaped.text.as_ref().find(src).unwrap_or(0);
            let start = run.start;
            let width = shaped.width;
            let mut boxes: Vec<(usize, Pixels, Pixels)> = Vec::new();
            let glyphs: Vec<&gpui::ShapedGlyph> =
                shaped.runs.iter().flat_map(|r| r.glyphs.iter()).collect();
            for (k, g) in glyphs.iter().enumerate() {
                let right = glyphs.get(k + 1).map_or(width, |n| n.position.x);
                boxes.push((g.index, g.position.x, right));
            }
            // `shape_line_rtl` puts clusters in visual order and renumbers
            // their indices visually: a cluster at visual `v..v_next` holds
            // the logical bytes `len - v_next..len - v` of the shaped text.
            let total = shaped.len;
            let mut clusters: Vec<(usize, Pixels, Pixels)> = Vec::new();
            for &(v, l, r) in &boxes {
                match clusters.last_mut() {
                    Some(c) if c.0 == v => {
                        c.1 = c.1.min(l);
                        c.2 = c.2.max(r);
                    }
                    _ => clusters.push((v, l, r)),
                }
            }
            let logical: Vec<(usize, usize, Pixels, Pixels)> = clusters
                .iter()
                .enumerate()
                .map(|(k, &(v, l, r))| {
                    let next = clusters.get(k + 1).map_or(total, |c| c.0);
                    (total.saturating_sub(next), total.saturating_sub(v), l, r)
                })
                .collect();
            let text = &self.text;
            let x_of = |a: usize, b: usize| -> (Pixels, Pixels) {
                if a <= start && b >= start + src.len() {
                    return (at.x, at.x + width);
                }
                let mut ext: Option<(Pixels, Pixels)> = None;
                let (a, b) = (a.max(start), b.min(start + src.len()));
                for (i, ch) in text[a..b].char_indices() {
                    // Joiners and other format characters draw nothing.
                    if matches!(ch, '\u{200b}'..='\u{200f}' | '\u{2060}'..='\u{206f}' | '\u{feff}' | '\u{202a}'..='\u{202e}') {
                        continue;
                    }
                    let o = lead + a + i - start;
                    if let Some(&(_, _, l, r)) = logical.iter().find(|c| c.0 <= o && o < c.1) {
                        ext = Some(ext.map_or((l, r), |(el, er)| (el.min(l), er.max(r))));
                    }
                }
                let (l, r) = ext.unwrap_or((px(0.), px(0.)));
                (at.x + l, at.x + r)
            };
            let baseline =
                at.y + (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent;
            self.paint_decor(run.clone(), &x_of, baseline, dy, line, through, window);
            return;
        }
        let lead = shaped.text.as_ref().find(src).unwrap_or(0);
        let baseline = at.y + (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent;
        let start = run.start;
        let width = shaped.width;
        let x_one = |i: usize| -> Pixels {
            if i <= start {
                at.x + if lead == 0 { px(0.) } else { shaped.x_for_index(lead) }
            } else if i - start >= src.len() {
                // The run end: its advance, not the appended marker's.
                at.x + if lead + src.len() >= shaped.text.len() {
                    width
                } else {
                    shaped.x_for_index(lead + src.len())
                }
            } else {
                at.x + shaped.x_for_index(lead + i - start)
            }
        };
        let x_of = |a: usize, b: usize| (x_one(a), x_one(b));
        self.paint_decor(run.clone(), &x_of, baseline, dy, line, through, window);
    }

    /// Vertical shift of the decorating box of `group`: the underline keeps
    /// to its decorating box, not to a shifted descendant
    /// (`text-decoration-decorating-box-001`). The box's own shift is the
    /// one most of the decorated run carries.
    fn group_shift(&self, group: &std::ops::Range<usize>) -> Pixels {
        let total = group.end.saturating_sub(group.start);
        let mut tally: Vec<(Pixels, usize)> = Vec::new();
        let mut shifted = 0usize;
        for (r, v) in &self.shift_spans {
            let n = r.end.min(group.end).saturating_sub(r.start.max(group.start));
            if n == 0 {
                continue;
            }
            shifted += n;
            match tally.iter_mut().find(|(t, _)| t == v) {
                Some(slot) => slot.1 += n,
                None => tally.push((*v, n)),
            }
        }
        tally.push((px(0.), total.saturating_sub(shifted)));
        tally.iter().max_by_key(|(_, n)| *n).map_or(px(0.), |(v, _)| *v)
    }

    /// Lines of the decorations over `frag` (bytes of one painted fragment;
    /// `x_of` maps a byte to its x, `baseline` is the fragment's baseline in
    /// logical px before device snapping).
    pub(super) fn paint_decor(
        &self,
        frag: std::ops::Range<usize>,
        x_of: &dyn Fn(usize, usize) -> (Pixels, Pixels),
        baseline: Pixels,
        dy: Pixels,
        line: &std::ops::Range<usize>,
        through: bool,
        window: &mut Window,
    ) {
        let scale = window.scale_factor();
        let mut segs: Option<Vec<Seg>> = None;
        for span in &self.decor_spans {
            let (s, e) = (span.range.start.max(frag.start), span.range.end.min(frag.end));
            if s >= e {
                continue;
            }
            for (s, e) in self.skip_parts(span.skip_spaces, s, e, line) {
            let (xa, xb) = x_of(s, e);
            let (xa, xb) = (f32::from(xa) * scale, f32::from(xb) * scale);
            let (fx0, fx1) = (xa.min(xb), xa.max(xb));
            if fx1 <= fx0 {
                continue;
            }
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
                // Ends: `text-decoration-inset` (css-text-decor-4 §4.1).
                let (mut l, mut r) = (0.0f32, 0.0f32);
                match d.inset {
                    None => {
                        let a = (t / 2.0).clamp(1.0, 2.0);
                        (l, r) = (a, a);
                    }
                    Some([st, en]) if !(inset_zero(st) && inset_zero(en)) => {
                        let w = fx1 - fx0;
                        if d.clone {
                            let prev = s > item.group.start && s > line.start;
                            let line_end = line.start
                                + self.text[line.clone()].trim_end_matches([' ', 9u8 as char, 10u8 as char]).len();
                            let next = e < item.group.end && e < line_end;
                            if !prev {
                                l = inset_px(st, w, scale);
                            }
                            if !next {
                                r = inset_px(en, w, scale);
                            }
                        } else {
                            let segs = segs.get_or_insert_with(|| self.measure(window));
                            let before = self.decor_width(segs, item.group.start, s) * scale;
                            let after = self.decor_width(segs, e, item.group.end) * scale;
                            let total = before + w + after;
                            l = inset_for_fragment(inset_px(st, total, scale), before);
                            r = inset_for_fragment(inset_px(en, total, scale), after);
                        }
                    }
                    _ => {}
                }
                if self.wrap.rtl {
                    std::mem::swap(&mut l, &mut r);
                }
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
                let vertical =
                    self.selection_vertical.is_some() && self.vertical_central_baseline;
                let flip = vertical
                    && if d.over_lang {
                        d.position & crate::computed::UPOS_LEFT == 0
                    } else {
                        d.position & crate::computed::UPOS_RIGHT != 0
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
                    (under_em(if line { off.unwrap_or(0.0) } else { 0.0 }), t + 1.0, t + 1.0, true)
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
                            let gap = if off.is_none() { (t / 2.0).ceil().max(1.0) } else { 0.0 };
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
                    rows.push((text_top + 2.0 * asc_f / 3.0 - t / 2.0, (t + 1.0).floor(), 0.0, false));
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
    }

    /// `text-decoration-skip-spaces` (css-text-decor-4 §4.2): the parts of
    /// `s..e` that are decorated. Spacers are Zs characters except U+202F;
    /// `start`/`end` skip them at the start/end of the line, `all`
    /// everywhere (word separators too).
    fn skip_parts(
        &self,
        skip: u8,
        s: usize,
        e: usize,
        line: &std::ops::Range<usize>,
    ) -> Vec<(usize, usize)> {
        use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};
        let spacer = |c: char| {
            c != '\u{202f}' && c.general_category() == GeneralCategory::SpaceSeparator
        };
        if skip & 4 != 0 {
            let mut out = Vec::new();
            let mut cur: Option<usize> = None;
            for (i, c) in self.text[s..e].char_indices() {
                if spacer(c) {
                    if let Some(st) = cur.take() {
                        out.push((st, s + i));
                    }
                } else if cur.is_none() {
                    cur = Some(s + i);
                }
            }
            if let Some(st) = cur {
                out.push((st, e));
            }
            return out;
        }
        let (mut a, mut b) = (s, e);
        if skip & 1 != 0 {
            // Spacers from the line start up to `a` must all be spacers.
            let head = &self.text[line.start.min(a)..a];
            if head.chars().all(spacer) {
                let lead: usize =
                    self.text[a..b].chars().take_while(|c| spacer(*c)).map(char::len_utf8).sum();
                a += lead;
            }
        }
        if skip & 2 != 0 && a < b {
            let tail = &self.text[b..line.end.max(b)];
            if tail.chars().all(|c| spacer(c) || c == '\n') {
                let trail: usize = self.text[a..b]
                    .chars()
                    .rev()
                    .take_while(|c| spacer(*c))
                    .map(char::len_utf8)
                    .sum();
                b -= trail;
            }
        }
        if a < b { vec![(a, b)] } else { Vec::new() }
    }

    /// Inline size of the decorated text `a..b` over all lines (logical px):
    /// the fragments of a line end before its hanging or removed spaces.
    fn decor_width(&self, segs: &[Seg], a: usize, b: usize) -> f32 {
        const NL: char = 10 as char;
        const TAB: char = 9 as char;
        let mut w = 0.0;
        for line in &self.lines {
            let body = &self.text[line.range.clone()];
            let end = if self.wrap.keep_spaces {
                line.range.start + body.trim_end_matches(NL).len()
            } else {
                line.range.start + body.trim_end_matches([' ', TAB, NL]).len()
            };
            let (s, e) = (a.max(line.range.start), b.min(end));
            if s < e {
                w += f32::from(self.span(segs, s, e));
            }
        }
        w
    }

    /// Paint the patterned lines merged so far (end of a painted line).
    pub(super) fn flush_decor(&self, window: &mut Window) {
        let scale = window.scale_factor();
        let pending = std::mem::take(&mut *self.decor_pending.borrow_mut());
        for line in pending {
            line.paint(scale, window);
        }
    }

    /// Ink boxes of the characters `s..e` in device px: (left, right, top,
    /// bottom), the vertical ones from the baseline (y down). Glyph boxes
    /// stand in for Blink's outline intercepts (`GetTextIntercepts`); CJK is
    /// excluded as in `kExcludeCJK`.
    fn ink_boxes(
        &self,
        s: usize,
        e: usize,
        x_of: &dyn Fn(usize, usize) -> (Pixels, Pixels),
        window: &mut Window,
    ) -> Vec<(f32, f32, f32, f32)> {
        let scale = window.scale_factor();
        let ts = window.text_system().clone();
        let mut out = Vec::new();
        let mut bounds: Vec<(std::ops::Range<usize>, &TextRun)> = Vec::new();
        let mut acc = 0usize;
        for r in &self.runs {
            bounds.push((acc..acc + r.len, r));
            acc += r.len;
        }
        for (i, ch) in self.text[s..e].char_indices() {
            let at = s + i;
            let Some((_, run)) = bounds.iter().find(|(r, _)| r.contains(&at)) else {
                continue;
            };
            if ch.is_whitespace() || is_cjk(ch) {
                continue;
            }
            let id = ts.resolve_font(&run.font);
            let size = run.font_size.unwrap_or(self.font_size);
            let Ok(b) = ts.typographic_bounds(id, size, ch) else {
                continue;
            };
            if b.size.width <= px(0.) || b.size.height <= px(0.) {
                continue;
            }
            let (xa, xb) = x_of(at, at + ch.len_utf8());
            let x = f32::from(xa.min(xb)) * scale;
            let left = x + f32::from(b.origin.x) * scale;
            let right = left + f32::from(b.size.width) * scale;
            // `origin.y` is the ink bottom with y up.
            let bottom = -f32::from(b.origin.y) * scale;
            let top = bottom - f32::from(b.size.height) * scale;
            out.push((left, right, top, bottom));
        }
        out
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x11FF | 0x2E80..=0x9FFF | 0xA960..=0xA97F | 0xAC00..=0xD7FF
        | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF | 0x20000..=0x3FFFF)
}

/// One decoration line in device px (Blink `DecorationGeometry`).
#[derive(Clone, Debug)]
pub struct DecorLine {
    style: DecorStyle,
    x0: f32,
    x1: f32,
    /// Top of the line rect, unsnapped.
    y: f32,
    t: f32,
    double_offset: f32,
    wavy_offset: f32,
    color: Hsla,
}

impl DecorLine {
    fn joins(&self, next: &DecorLine) -> bool {
        self.style == next.style
            && self.y == next.y
            && self.t == next.t
            && self.color == next.color
            && (next.x0 - self.x1).abs() < 0.01
    }

    fn paint(&self, scale: f32, window: &mut Window) {
        paint_line_style(
            self.style,
            self.x0,
            self.x1,
            self.y,
            self.t,
            self.double_offset,
            self.wavy_offset,
            self.color,
            scale,
            window,
        );
    }
}

fn inset_zero(l: DecorLen) -> bool {
    match l {
        DecorLen::Px(v) | DecorLen::Pct(v) => v == 0.0,
        DecorLen::Mix(k, v) => k == 0.0 && v == 0.0,
        _ => true,
    }
}

/// Inset in device px; a percentage refers to `basis` (device px).
fn inset_px(l: DecorLen, basis: f32, scale: f32) -> f32 {
    match l {
        DecorLen::Px(v) => v * scale,
        DecorLen::Pct(k) => k * basis,
        DecorLen::Mix(k, v) => k * basis + v * scale,
        _ => 0.0,
    }
}

/// Blink `ResolveInsetForFragment`: with `slice`, a positive inset trims the
/// run from its start and spills over the following fragments, a negative one
/// extends only the outer end.
fn inset_for_fragment(inset: f32, preceding: f32) -> f32 {
    if inset <= 0.0 {
        if preceding == 0.0 { inset } else { 0.0 }
    } else {
        (inset - preceding).max(0.0)
    }
}

/// Rect in device px → logical quad.
fn dev_quad(x0: f32, x1: f32, y: f32, h: f32, color: Hsla, scale: f32, window: &mut Window) {
    if x1 <= x0 || h <= 0.0 {
        return;
    }
    window.paint_quad(gpui::fill(
        Bounds {
            origin: point(px(x0 / scale), px(y / scale)),
            size: size(px((x1 - x0) / scale), px(h / scale)),
        },
        color,
    ));
}

/// One decoration line (Blink `DecorationLinePainter::Paint`): `y` is the
/// top of the line rect in device px, `double_offset` the second line of
/// `double`, `wavy_offset` the shift of the wave.
#[allow(clippy::too_many_arguments)]
fn paint_line_style(
    style: DecorStyle,
    x0: f32,
    x1: f32,
    y: f32,
    t: f32,
    double_offset: f32,
    wavy_offset: f32,
    color: Hsla,
    scale: f32,
    window: &mut Window,
) {
    // `SnapYAxis`: nearest device row, thickness rounded down.
    let h = t.floor().max(1.0);
    match style {
        DecorStyle::Solid | DecorStyle::Double => {
            dev_quad(x0, x1, (y + 0.5).floor(), h, color, scale, window);
            if style == DecorStyle::Double {
                dev_quad(x0, x1, (y + double_offset + 0.5).floor(), h, color, scale, window);
            }
        }
        DecorStyle::Dotted | DecorStyle::Dashed => {
            // `DrawLineAsStroke`: the stroke centre on a device row.
            let ti = t.round().max(1.0);
            let mid = (y + (t / 2.0).max(0.5)).floor();
            let top = if ti as i32 % 2 == 1 {
                mid + 0.5 - ti / 2.0
            } else {
                mid - ti / 2.0
            };
            let (dash, gap) = if style == DecorStyle::Dashed {
                (3.0 * ti, 3.0 * ti)
            } else {
                (ti, ti)
            };
            let mut x = x0;
            while x < x1 {
                let end = (x + dash).min(x1);
                if style == DecorStyle::Dotted && ti >= 3.0 {
                    window.paint_quad(
                        gpui::fill(
                            Bounds {
                                origin: point(px(x / scale), px(top / scale)),
                                size: size(px((end - x) / scale), px(ti / scale)),
                            },
                            color,
                        )
                        .corner_radii(px(ti / 2.0 / scale)),
                    );
                } else {
                    dev_quad(x, end, top, ti, color, scale, window);
                }
                x += dash + gap;
            }
        }
        DecorStyle::Wavy => {
            // Blink `MakeWave`: the wave sits below an underline and above
            // an overline by the thickness plus one.
            let amp = (0.5 + (3.0 * t + 0.5).round()) * 0.29;
            let centre = y + 0.5 + wavy_offset;
            window.paint_underline(
                point(px(x0 / scale), px((centre - amp - t / 2.0) / scale)),
                px((x1 - x0) / scale),
                &gpui::UnderlineStyle {
                    thickness: px(t / scale),
                    color: Some(color),
                    wavy: true,
                },
            );
        }
    }
}
