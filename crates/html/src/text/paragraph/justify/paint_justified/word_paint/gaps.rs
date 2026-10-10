//! Gaps for word_paint; split out to keep the owning module within 250 lines.

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
                if (want - gap_shaped.width).abs() > px(0.5) {
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
                let gap_y = match (line_base, self.base_of(&gap)) {
                    (Some(l), Some(w)) => y + px(l - w),
                    _ => y,
                };
                self.paint_run_background(&gap_shaped, point(gap_x, gap_y), window, cx);
            }
        }
        // Растянутый выключкой пробел тоже принадлежит прогону, и его
        // подложка обязана быть сплошной. Красим ТОЛЬКО когда пробел
        // целиком внутри одного прогона с фоном — иначе фон соседнего
        // куска растекается по чужому месту (замерено: css-text 1015 →
        // 691 при покраске каждого промежутка).
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
