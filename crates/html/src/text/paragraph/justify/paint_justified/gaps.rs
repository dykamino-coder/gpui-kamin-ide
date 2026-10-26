//! Подложка промежутков между словами на пути отрисовки по словам: полоса
//! строчной коробки продолжается сквозь пробелы — нерастянутый промежуток
//! набирается своими прогонами по ширине разложенной строки (с
//! `word-spacing`), растянутый выключкой красится только внутри одного
//! прогона с фоном; для rtl-строки — по обращённым друг к другу краям
//! соседей и висящие пробелы за левым краем (css-text-3 §4.1.3).

use super::JustifiedPaint;
use super::{Edge, Word};
use crate::text::paragraph::*;
use gpui::{App, Bounds, Pixels, SharedString, Window, point, px};

impl Paragraph {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_word_gap(
        &self,
        context: JustifiedPaint<'_>,
        word: &Word,
        wi: usize,
        x: Pixels,
        dy: Pixels,
        shaped_width: Pixels,
        line_base: Option<f32>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let JustifiedPaint {
            segs,
            words,
            ltr_place,
            step,
            from,
            bounds,
            dx,
            y,
            pad,
            placed,
            ..
        } = context;
        // Пробелы между словами тоже принадлежат полосе коробки: без
        // этого фон и рамка `<span>` рвались на каждом пробеле. Промежуток
        // набирается своими прогонами (обе стороны — продолжение полосы) и
        // красит только подложку. Растянутые выключкой промежутки красит
        // ветка ниже.
        if step == px(0.)
            && !self.wrap.rtl
            && let Some(next) = words.get(wi + 1)
            && next.range.start > word.range.end
        {
            let gap = word.range.end..next.range.start;
            let gap_runs = slice_runs_banded(&self.runs, &gap);
            if gap_runs.iter().any(|r| r.background_color.is_some()) {
                let gap_text: SharedString = self.text[gap.clone()].to_string().into();
                let spacing = self
                    .letter_spans
                    .iter()
                    .find(|(r, _)| r.contains(&gap.start))
                    .map(|(_, v)| *v)
                    .unwrap_or(self.letter_spacing);
                let mut gap_shaped = window
                    .text_system()
                    .with_ligature_breaking(false)
                    .shape_line_spaced(gap_text.clone(), self.font_size, &gap_runs, None, spacing);
                // Ширина промежутка — по РАЗЛОЖЕННОЙ строке: там в нём уже
                // лежит `word-spacing`, а отдельный набор пробела его не
                // знает, и полоса `<span>` рвалась на каждом растянутом
                // пробеле (`word-spacing-characters-001`). Недостача
                // раздаётся трекингом по знакам промежутка.
                let want =
                    self.x_at(segs, gap.end, Edge::Start) - self.x_at(segs, gap.start, Edge::Start);
                let n = gap_text.chars().count().max(1) as f32;
                if (want - gap_shaped.width).abs() > px(crate::tolerance::SHAPED_ADVANCE_EPS) {
                    gap_shaped = window
                        .text_system()
                        .with_ligature_breaking(false)
                        .shape_line_spaced(
                            gap_text,
                            self.font_size,
                            &gap_runs,
                            None,
                            spacing + (want - gap_shaped.width) / n,
                        );
                }
                let gap_x = bounds.origin.x
                    + dx
                    + placed(gap.start, gap.end)
                        .map_or(self.x_at(segs, gap.start, Edge::Start) - from, |v| v.0);
                // The gap is a piece of the same inline box as the words
                // around it: it takes their edge alignment too, or its band
                // steps against theirs (CSS 2.1 sections 10.8.1 and 14.2).
                let gap_y = y + self.piece_fix(&gap, line_base, pad);
                self.paint_run_background(&gap_shaped, point(gap_x, gap_y), window, cx);
            }
        }
        // Растянутый выключкой пробел тоже принадлежит прогону, и его
        // подложка обязана быть сплошной. Красим ТОЛЬКО когда пробел
        // целиком внутри одного прогона с фоном — иначе фон соседнего
        // куска растекается по чужому месту (покраска каждого промежутка
        // ломает большую часть семьи css-text).
        if step > px(0.)
            && !self.wrap.rtl
            && ltr_place.is_empty()
            && let Some(next) = words.get(wi + 1)
            && next.range.start > word.range.end
            && let Some(bg) = self.gap_background(word, next)
        {
            let after = (self.x_at(segs, next.range.start, Edge::Start) - from)
                + step * next.spaces_before as f32;
            let right = bounds.origin.x + dx + after;
            let left = x + shaped_width;
            if right > left {
                window.paint_quad(gpui::fill(
                    Bounds {
                        origin: point(left, y + dy),
                        size: gpui::size(right - left, self.line_height),
                    },
                    bg,
                ));
            }
        }
    }
}

impl Paragraph {
    /// Right-to-left counterpart of the stretched gap fill (the left-to-right
    /// branch above). Words of a right-to-left line are placed by the bidi
    /// visual order, so a logically adjacent pair may sit either way round:
    /// the fill spans whichever edges face each other. After the last word,
    /// the line's trailing spaces (hanging under `pre-wrap`, css-text-3
    /// §4.1.3) keep their natural advance and box background at the visual
    /// line end, i.e. left of the leftmost word, overflowing the line start
    /// like any right-to-left content (Blink paints hanging spaces with
    /// their text fragment; UAX #9 L1 puts trailing white space at the
    /// paragraph level).
    pub(crate) fn paint_rtl_gaps(
        &self,
        context: JustifiedPaint<'_>,
        boxes: &[(Pixels, Pixels, Pixels)],
        window: &mut Window,
    ) {
        let JustifiedPaint {
            range,
            segs,
            words,
            y,
            ..
        } = context;
        let mut fill = |left: Pixels, right: Pixels, dy: Pixels, bg: gpui::Hsla| {
            if right > left {
                window.paint_quad(gpui::fill(
                    Bounds {
                        origin: point(left, y + dy),
                        size: gpui::size(right - left, self.line_height),
                    },
                    bg,
                ));
            }
        };
        for (wi, pair) in words.windows(2).enumerate() {
            let (word, next) = (&pair[0], &pair[1]);
            if next.range.start <= word.range.end {
                continue;
            }
            let Some(bg) = self.gap_background(word, next) else {
                continue;
            };
            let ((a, aw, dy), (b, bw, _)) = (boxes[wi], boxes[wi + 1]);
            if a + aw <= b {
                fill(a + aw, b, dy, bg);
            } else if b + bw <= a {
                fill(b + bw, a, dy, bg);
            }
        }
        let (Some(last), Some(&(_, _, dy))) = (words.last(), boxes.last()) else {
            return;
        };
        if range.end <= last.range.end
            || !self.text[last.range.end..range.end]
                .chars()
                .all(|c| c == ' ')
        {
            return;
        }
        let tail = Word {
            range: range.end - 1..range.end,
            spaces_before: 0,
        };
        let Some(bg) = self.gap_background(last, &tail) else {
            return;
        };
        let w =
            self.x_at(segs, range.end, Edge::Start) - self.x_at(segs, last.range.end, Edge::Start);
        let left = boxes
            .iter()
            .map(|b| b.0)
            .fold(boxes[0].0, |m, v| if v < m { v } else { m });
        fill(left - w, left, dy, bg);
    }
}
