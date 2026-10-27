//! Статический прямоугольник горизонтальной строки: её шаг, выключка и UAX #9.

use crate::text::paragraph::*;
use gpui::{Bounds, Pixels, point, px, size};

pub(super) enum Position {
    Inline,
    Block,
    AfterBreak,
}

pub(super) fn rect(
    p: &Paragraph,
    segs: &[Seg],
    pads: &[(f32, f32)],
    at: usize,
    width: Pixels,
    kind: Position,
) -> (Bounds<Pixels>, Bounds<Pixels>) {
    let row = p
        .lines
        .iter()
        .position(|l| at < l.range.end)
        .unwrap_or(p.lines.len().saturating_sub(1));
    let step = |i: usize| {
        let (a, b) = pads.get(i).copied().unwrap_or_default();
        p.line_height + px(a + b)
    };
    let before: Pixels = (0..row).map(step).fold(px(0.), |a, b| a + b);
    let clearance = p.lines.get(row).map_or(px(0.), |l| l.clearance);
    let top = before + clearance;
    let height = step(row) - clearance;
    let line = Bounds {
        origin: point(px(0.), top),
        size: size(width, height),
    };
    let Some(l) = p.lines.get(row) else {
        return empty(p, width, px(0.), 0, matches!(kind, Position::Block));
    };
    if matches!(kind, Position::AfterBreak) {
        return empty(p, width, top + height, row + 1, false);
    }
    if matches!(kind, Position::Block) {
        // Гипотетический блок прерывает предшествующую строку. Его rect
        // охватывает inline-ось CB, а не высоту нулевого inline-щупа.
        let before = at.clamp(l.range.start, l.range.end);
        let preceded = p.text[l.range.start..before]
            .chars()
            .any(|c| !matches!(c, '\u{feff}' | '\u{200b}'));
        let y = top + if preceded { height } else { px(0.) };
        return (
            Bounds {
                origin: point(px(0.), y),
                size: size(width, px(0.)),
            },
            line,
        );
    }
    let align = p.line_align(row, l);
    let free = width - l.width - l.indent - px(p.flow_cut(row).1);
    let hang = p.hang_first(l.range.start);
    let lead = if p.wrap.rtl { px(0.) } else { l.indent }
        - p.span(segs, l.range.start, l.range.start + hang);
    let left = line_offset(align, p.wrap.rtl, free) + lead;
    let range = l.range.start..l.range.start + p.text[l.range.clone()].trim_end_matches('\n').len();
    let (_, _, stretch, absorbed, from) = p.prepare_words(
        &range,
        segs,
        if align == Align::Justify {
            free.max(px(0.))
        } else {
            px(0.)
        },
    );
    let advance = |pos: usize| {
        p.x_at(segs, pos, Edge::Start) - from
            + stretch * p.justify_opps(&range, pos).saturating_sub(absorbed) as f32
    };
    let x = left + visual(p, at, l, &advance);
    (
        Bounds {
            origin: point(x, top),
            size: size(px(0.), height),
        },
        line,
    )
}

fn empty(
    p: &Paragraph,
    width: Pixels,
    top: Pixels,
    row: usize,
    block: bool,
) -> (Bounds<Pixels>, Bounds<Pixels>) {
    let line = Bounds {
        origin: point(px(0.), top),
        size: size(width, px(0.)),
    };
    if block {
        return (line, line);
    }
    let (left, right) = p.flow_cut(row);
    let indent = p.indent_of(true, row == 0, Some(width));
    let align = p.align_last.unwrap_or(match p.align {
        Align::Justify if p.wrap.rtl => Align::Right,
        Align::Justify => Align::Left,
        other => other,
    });
    let x = px(left)
        + if p.wrap.rtl { px(0.) } else { indent }
        + line_offset(align, p.wrap.rtl, width - indent - px(left + right));
    (
        Bounds {
            origin: point(x, top),
            size: size(px(0.), px(0.)),
        },
        line,
    )
}

fn visual(p: &Paragraph, at: usize, line: &Line, advance: &impl Fn(usize) -> Pixels) -> Pixels {
    let start = line.range.start;
    let end = start + trim_hanging(&p.text[line.range.clone()]);
    let at = at.clamp(start, end);
    // Нейтральный объект даёт static point на границе bidi-прогонов,
    // даже когда она не совпадает с логическим концом предшественника.
    const MARK: usize = 3;
    let mut text = String::with_capacity(p.text.len() + MARK);
    text.push_str(&p.text[..at]);
    text.push('\u{fffc}');
    text.push_str(&p.text[at..]);
    let base = if p.wrap.rtl {
        unicode_bidi::Level::rtl()
    } else {
        unicode_bidi::Level::ltr()
    };
    let bidi = unicode_bidi::BidiInfo::new(&text, p.plaintext.is_none().then_some(base));
    let para = bidi
        .paragraphs
        .iter()
        .find(|p| p.range.start <= start && start < p.range.end)
        .expect("static-position marker belongs to a bidi paragraph");
    let (levels, runs) = bidi.visual_runs(para, start..end + MARK);
    let original = |pos: usize| if pos <= at { pos } else { pos - MARK };
    let span = |a, b| advance(original(b)) - advance(original(a));
    let mut x = px(0.);
    for run in runs {
        if run.contains(&at) {
            return x + if levels[run.start].is_rtl() {
                span(at + MARK, run.end)
            } else {
                span(run.start, at)
            };
        }
        x += span(run.start, run.end);
    }
    unreachable!("static-position marker belongs to a visual run")
}

#[cfg(test)]
mod tests;
