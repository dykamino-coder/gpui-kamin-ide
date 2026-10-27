//! Отрисовка слов строки (`paint_words`): прогоны, набор, место по раздаче или зеркалу,
//! сдвиги `vertical-align`/`position: relative`, подложка, оформление, знаки акцента;
//! вертикальная поправка отдельно набранного куска (CSS 2.1 §10.8.1).

use super::Edge;
use super::JustifiedPaint;
use crate::text::paragraph::*;
use gpui::{App, Pixels, SharedString, Window, point, px};

impl Paragraph {
    pub(crate) fn paint_words(
        &self,
        context: JustifiedPaint<'_>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let JustifiedPaint {
            range,
            segs,
            words,
            ltr_place,
            logical_run,
            step,
            from,
            mirror,
            bounds,
            dx,
            y,
            pad,
            logical_at,
            placed,
        } = context;
        let line_base = self.shared_base(range);
        let mut decor_prev: Option<(usize, Pixels, Pixels, Pixels)> = None;
        let mut boxes: Vec<(Pixels, Pixels, Pixels)> = Vec::with_capacity(words.len());
        for (wi, word) in words.iter().enumerate() {
            let slice: SharedString = self.text[word.range.clone()].to_string().into();
            // Полоса строчной коробки продолжается сквозь слова (см.
            // `slice_runs_banded`); при rtl слова зеркалятся, и стороны
            // меняются местами — там прежний счёт.
            let mut runs = match ltr_place
                .iter()
                .position(|p| p.0 <= word.range.start && word.range.start < p.1)
            {
                Some(k) => self.visual_band_runs(&ltr_place, k, &word.range),
                None if self.wrap.rtl => self.mirrored_band_runs(&word.range),
                None => slice_runs_banded(&self.runs, &word.range),
            };
            let decor = self.decor_on();
            if decor {
                for r in runs.iter_mut() {
                    r.underline = None;
                    r.strikethrough = None;
                }
            }
            let spacing = self
                .letter_spans
                .iter()
                .find(|(r, _)| r.contains(&word.range.start))
                .map(|(_, v)| *v)
                .unwrap_or(self.letter_spacing);
            let visual = placed(word.range.start, word.range.end);
            let overridden = runs
                .iter()
                .any(|r| crate::text::fonts::metric_overrides::has_overrides(&r.font));
            let shaped = if visual.map_or(self.wrap.rtl, |v| v.1) {
                // A right-to-left level piece: glyphs in visual order (UAX #9
                // L2/L4), shaped once under an RLO like `shape`.
                let mut runs = runs;
                let body = controlled_shape::text(&slice, &mut runs, true);
                window
                    .text_system()
                    .with_ligature_breaking(false)
                    .shape_line_rtl(body, self.font_size, &runs, spacing)
            } else {
                window
                    .text_system()
                    .with_ligature_breaking(false)
                    .shape_line_spaced(slice, self.font_size, &runs, None, spacing)
            };
            let logical = (self.x_at(segs, word.range.start, Edge::Start) - from)
                + step * word.spaces_before as f32;
            // При письме справа налево строка раздаётся от ПРАВОГО края:
            // первое слово встаёт справа, последнее — слева. Раздача слева
            // направо переворачивала порядок слов на выключенной строке.
            let x = match logical_run.iter().find(|(i, _)| *i == wi) {
                Some((_, fixed)) => *fixed,
                // A right-to-left line starts at its mirror axis: its
                // logical extent ends at the visual left.
                None if visual.is_some() => {
                    let left = if self.wrap.rtl {
                        mirror - logical_at(range.end)
                    } else {
                        bounds.origin.x + dx
                    };
                    left + visual.map_or(px(0.), |v| v.0)
                }
                None if self.wrap.rtl => mirror - logical - shaped.width,
                None => bounds.origin.x + dx + logical,
            };
            // Сдвиг куска по вертикали: надстрочный и подстрочный знак стоят
            // выше и ниже базовой линии, оставаясь в той же строке.
            let dy = self
                .shift_spans
                .iter()
                .find(|(r, _)| r.contains(&word.range.start))
                .map(|(_, v)| *v)
                .unwrap_or(px(0.));
            // Относительный сдвиг двигает ТОЛЬКО отрисовку куска: место в
            // потоке за ним сохраняется, соседи не съезжают (§9.4.3).
            let ((rx, ry), (bx, by)) = self
                .rel_spans
                .iter()
                .find(|(r, _, _)| r.contains(&word.range.start))
                .map(|(_, v, b)| (*v, *b))
                .unwrap_or(((0.0, 0.0), (0.0, 0.0)));
            // Слово набирается своим вызовом, и `ShapedLine::paint` ставит
            // его базовую линию по СВОИМ подъёму и спуску. Сплошной набор
            // строки берёт наибольшие по всей строке — куски разного кегля
            // стоят на одной базовой линии (§10.8): слово опускается на разницу.
            let word_base = if overridden {
                Some(f32::from(
                    (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent,
                ))
            } else {
                self.base_of(&word.range)
            };
            let fix = self.piece_fix_with(&word.range, word_base, line_base, pad);
            let at = point(x + px(rx), y + dy + px(ry) + fix);
            // Подложка прогона — отдельным вызовом, см. выше. Она — коробка
            // своего строчного элемента и едет его сдвигом, а не сдвигом
            // относительного потомка (`position-relative-033`).
            let box_at = point(at.x - px(rx - bx), at.y - px(ry - by));
            self.paint_run_background(&shaped, box_at, window, cx);
            let origin = self.text_raster_origin(&shaped, at, window);
            if decor {
                // Украшения промежутка до слова: от правого края прошлого
                // слова до левого края этого (растянутый пробел тоже
                // украшается, css-text-decor-3 §2.1).
                if let Some((end, right, base, pdy)) = decor_prev
                    && end < word.range.start
                    && !self.wrap.rtl
                {
                    let left = at.x;
                    let gap = end..word.range.start;
                    let x_of = |_: usize, _: usize| (right, left);
                    self.paint_decor(gap.clone(), &x_of, base, pdy, range, false, window);
                    self.paint_decor(gap.clone(), &x_of, base, pdy, range, true, window);
                }
                self.paint_decor_shaped(&word.range, &shaped, at, dy, false, range, false, window);
            }
            let _ = shaped.paint(
                origin,
                self.line_height,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            );
            if decor {
                self.paint_decor_shaped(&word.range, &shaped, at, dy, false, range, true, window);
                let base = at.y
                    + (self.line_height - shaped.ascent - shaped.descent) / 2.0
                    + shaped.ascent;
                decor_prev = Some((word.range.end, at.x + shaped.width, base, dy));
            }
            if !self.emph_spans.is_empty() {
                self.paint_emphasis(
                    &word.range,
                    &shaped,
                    at,
                    visual.is_some_and(|v| v.1),
                    window,
                    cx,
                );
            }
            boxes.push((x, shaped.width, dy));
            self.paint_word_gap(
                context,
                word,
                wi,
                x,
                dy,
                shaped.width,
                line_base,
                window,
                cx,
            );
        }
        if self.wrap.rtl && step > px(0.) {
            self.paint_rtl_gaps(context, &boxes, window);
        }
        if self.decor_on() {
            self.flush_decor(window);
        }
    }

    /// Vertical correction of a separately shaped piece of the line: its own
    /// baseline is moved onto the line's, or, inside a top/bottom aligned
    /// inline box, onto that box's edge (CSS 2.1 section 10.8.1). Words and
    /// the inter-word gaps of the same box share it, so their backgrounds
    /// form one band.
    pub(super) fn piece_fix(
        &self,
        range: &std::ops::Range<usize>,
        line_base: Option<f32>,
        pad: (f32, f32),
    ) -> Pixels {
        self.piece_fix_with(range, self.base_of(range), line_base, pad)
    }

    /// `piece_fix` with the caller's baseline (font metric overrides).
    pub(super) fn piece_fix_with(
        &self,
        range: &std::ops::Range<usize>,
        piece_base: Option<f32>,
        line_base: Option<f32>,
        pad: (f32, f32),
    ) -> Pixels {
        let fix = match (line_base, piece_base) {
            (Some(l), Some(w)) => px(l - w),
            _ => px(0.),
        };
        // Кусок у края строки: его строчная коробка (высотой своей
        // `line-height`, глифы по её полулидингу) встаёт верхом на верх
        // строки или низом на низ (§10.8.1). `shaped.paint` центрирует
        // глифы в высоте строки абзаца — разница высот делится пополам.
        match self
            .edge_spans
            .iter()
            .find(|(r, _, _)| r.contains(&range.start))
        {
            Some((r, top, h)) => {
                let lh = f32::from(self.line_height);
                let line_top = -pad.0;
                let box_top = if *top { line_top } else { lh + pad.1 - h };
                // Align the subtree as one box, preserving its common
                // baseline (CSS 2.1 section 10.8.1). Centering each word
                // separately misaligns descendants with different fonts.
                let baseline = match (self.base_of(r), piece_base) {
                    (Some(group), Some(word)) => group - word,
                    _ => 0.0,
                };
                px(box_top + (h - lh) / 2.0 + baseline)
            }
            None => fix,
        }
    }
}
