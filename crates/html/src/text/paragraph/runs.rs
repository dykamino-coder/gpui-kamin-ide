//! Прогоны: обрезка, управляющие bidi, соседи, нарезка по полосам.

use crate::text::paragraph::*;

/// Сторона письма по ПЕРВОМУ СИЛЬНОМУ знаку куска: `Some(true)` — справа
/// налево, `None` — сильных знаков нет вовсе.
pub(crate) fn first_strong_rtl(text: &str) -> Option<bool> {
    for ch in text.chars() {
        match unicode_bidi::bidi_class(ch) {
            unicode_bidi::BidiClass::L => return Some(false),
            unicode_bidi::BidiClass::R | unicode_bidi::BidiClass::AL => return Some(true),
            _ => {}
        }
    }
    None
}

/// Знак-указание, у которого нет своего изображения: словосоединитель,
/// нулевой пробел, метка порядка байтов, мягкий перенос, знаки управления
/// встроенностью.
pub(crate) fn invisible(ch: char) -> bool {
    matches!(ch as u32,
        0x00AD | 0x200B | 0x2060 | 0xFEFF | 0x202A..=0x202E | 0x2066..=0x2069)
}

/// Прогоны без невидимых знаков: длины считаются по оставшимся байтам.
pub(crate) fn trim_runs(runs: &[TextRun], text: &str) -> Vec<TextRun> {
    let mut out = Vec::with_capacity(runs.len());
    let mut at = 0usize;
    for run in runs {
        let end = (at + run.len).min(text.len());
        let kept: usize = text
            .get(at..end)
            .map(|s| {
                s.chars()
                    .filter(|c| !invisible(*c))
                    .map(char::len_utf8)
                    .sum()
            })
            .unwrap_or(0);
        if kept > 0 {
            let mut piece = run.clone();
            piece.len = kept;
            out.push(piece);
        }
        at = end;
    }
    out
}

/// Куски оформления, попавшие в отрезок строки.
/// Прогоны отрезка для ПОСЛОВНОЙ отрисовки полосы строчной коробки.
///
/// Слово, вырезанное из середины `<span>` с фоном или рамкой, — не начало и не
/// конец коробки: полоса продолжается в соседние знаки той же коробки, и поле
/// с боковой гранью на этой стороне не ставится (css-break-3
/// `box-decoration-break: slice`; на переносе то же делает сплошной набор,
/// `vendor/gpui` `line.rs` `run_background_quad` `pad_left/pad_right`).
/// Прежде каждое слово рисовало полную коробку — с рамкой и полем с обеих
/// сторон, и `<span>` с рамкой распадался на коробки по словам.
///
/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (03.10): полоса для `<span>` с рамкой БЕЗ фона —
/// прозрачная подложка прогона плюс `inline_pad` в `inline.rs`, чтобы
/// `run_background_quad` рисовал и такую рамку. Даже с отсечкой rtl,
/// `unicode-bidi: bidi-override`, сильных R/AL и знаков направления срез 3000
/// пар дал +1/−14, срез 125 строчных пар +7/−16: теряет семья `bidi-*` —
/// пословная отрисовка ltr-абзаца со знаками RLO/LRO идёт в ЛОГИЧЕСКОМ
/// порядке, и полоса на каждый видимый прогон рисует боковые грани дважды.
/// Возвращаться вместе с двунаправленной раскладкой полос (box-decoration по
/// видимым фрагментам, css-break-3 §5.4).
/// Zero-width bidi formatting characters (UAX #9 explicit formatting and
/// implicit marks).
pub(crate) fn bidi_control(c: char) -> bool {
    matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

/// Visual neighbours (byte offsets) of the characters `range` in the line's
/// visual character order `vis`, skipping zero-width bidi controls.
pub(crate) fn visual_neighbours(
    text: &str,
    vis: &[usize],
    range: &std::ops::Range<usize>,
) -> (Option<usize>, Option<usize>) {
    let mut own = vis.iter().enumerate().filter(|(_, p)| range.contains(p)).map(|(i, _)| i);
    let Some(first) = own.next() else {
        return (None, None);
    };
    let (lo, hi) = own.fold((first, first), |(a, b), i| (a.min(i), b.max(i)));
    let visible = |p: &&usize| !text[**p..].chars().next().is_some_and(bidi_control);
    let left = vis[..lo].iter().rev().find(visible).copied();
    let right = vis[hi + 1..].iter().find(visible).copied();
    (left, right)
}

/// Drop the physical left and/or right side (padding and border) of a run's
/// inline box band.
pub(crate) fn cut_band_sides(run: &mut TextRun, left: bool, right: bool) {
    for (cut, side) in [(left, 3), (right, 1)] {
        if cut {
            run.background_pad[side] = px(0.);
            if let Some(b) = run.background_border.as_mut() {
                b.1[side] = px(0.);
            }
        }
    }
}

pub(crate) fn slice_runs_banded(runs: &[TextRun], range: &std::ops::Range<usize>) -> Vec<TextRun> {
    let mut out = slice_runs(runs, range);
    let band_at = |at: usize| -> Option<(Option<Hsla>, Option<(Hsla, [Pixels; 4])>)> {
        let mut start = 0usize;
        for run in runs {
            if at < start + run.len {
                return Some((run.background_color, run.background_border));
            }
            start += run.len;
        }
        None
    };
    // Both ends are judged on the runs as sliced: cutting the left side of a
    // one-run slice first made its right side differ from the neighbour, and
    // a band continued on both sides kept its right side (a bar after every
    // tab-separated word of a bordered `<span>`, `tab-bidi-001`). The cut
    // covers the whole band segment at that end: `vendor/gpui` draws a band
    // with the sides of its first run.
    let band = |r: &TextRun| (r.background_color, r.background_border);
    let n = out.len();
    let cut_left = range.start > 0
        && out
            .first()
            .is_some_and(|f| f.background_color.is_some() && band_at(range.start - 1) == Some(band(f)));
    let cut_right = out
        .last()
        .is_some_and(|l| l.background_color.is_some() && band_at(range.end) == Some(band(l)));
    let mut cuts: Vec<(usize, bool)> = Vec::new();
    if cut_left {
        let own = band(&out[0]);
        cuts.extend((0..n).take_while(|&i| band(&out[i]) == own).map(|i| (i, true)));
    }
    if cut_right {
        let own = band(&out[n - 1]);
        cuts.extend((0..n).rev().take_while(|&i| band(&out[i]) == own).map(|i| (i, false)));
    }
    for (i, left) in cuts {
        cut_band_sides(&mut out[i], left, !left);
    }
    out
}

pub(crate) fn slice_runs(runs: &[TextRun], range: &std::ops::Range<usize>) -> Vec<TextRun> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for run in runs {
        let end = at + run.len;
        let from = at.max(range.start);
        let to = end.min(range.end);
        if from < to {
            let mut piece = run.clone();
            piece.len = to - from;
            out.push(piece);
        }
        at = end;
        if at >= range.end {
            break;
        }
    }
    out
}
