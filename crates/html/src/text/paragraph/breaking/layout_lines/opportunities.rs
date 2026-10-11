//! Opportunities for layout_lines; split out to keep the owning module within 250 lines.

use crate::text::paragraph::*;
use gpui::px;

impl Paragraph {
    pub(crate) fn part_opportunities(&self, part: &std::ops::Range<usize>) -> Vec<Stop> {
        let mut opportunities: Vec<Stop> = self
            .opportunities()
            .into_iter()
            .filter(|s| s.at > part.start && s.at <= part.end)
            .collect();
        // Конец текста — тоже точка проверки: без него хвост последней строки
        // никто не мерил и она оставалась во всю длину, сколько бы ни
        // переполняла коробку.
        if opportunities.last().is_none_or(|s| s.at < part.end) {
            opportunities.push(Stop {
                at: part.end,
                mandatory: false,
            });
        }
        opportunities
    }
}

impl Paragraph {
    pub(crate) fn candidate_width(
        &self,
        start: usize,
        at: usize,
        part_end: usize,
        segs: &[Seg],
        limit: Option<gpui::Pixels>,
    ) -> (usize, gpui::Pixels, bool) {
        // Хвостовые пробелы висят за краем: в ширину строки они не входят.
        // Хвостовые пробелы висят за краем СТРОКИ — то есть когда край
        // вообще есть. При замере по максимальному содержимому предела
        // нет, и сохранённый пробел в ширину ВХОДИТ (`pre-wrap-017`:
        // коробка `width: max-content` выходила на знак уже).
        let measured = if self.spaces_are_content() || (limit.is_none() && self.wrap.keep_spaces) {
            at
        } else {
            self.hang_tail(start, at)
        };
        // Свисающее за края в ширину строки не входит — ни открывающий
        // знак в начале, ни точка с запятой в конце.
        let head = start + self.hang_first(start);
        // Свисает ли знак — зависит от того, влезает ли строка БЕЗ него;
        // поэтому ширина считается дважды: сначала без свисания.
        let bare = self.span(segs, head, measured);
        let tight = limit.is_some_and(|w| bare > w);
        let tail_hang = self.hang_last(measured, at >= part_end, tight);
        let mut width =
            self.span(segs, head, measured - tail_hang) - self.tail_spacing(measured - tail_hang);
        // Строка, кончающаяся мягким переносом, несёт ещё и знак переноса.
        if self.text[..measured].ends_with('\u{00ad}') {
            width += self.hyphen_width(measured);
        }
        // Допуск в сотую точки: ширина строки складывается из замеров
        // кусков и знака переноса, и на ТОЧНОМ совпадении с коробкой
        // накопленная ошибка решала исход (`hyphens-manual-011`: строка,
        // влезающая ровно, уходила на перенос).
        let over = limit.is_some_and(|w| f32::from(width) > f32::from(w) + 0.01);
        (head, width, over)
    }
}

impl Paragraph {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn push_final_line(
        &self,
        out: &mut Vec<Line>,
        start: usize,
        part_end: usize,
        head_of_part: bool,
        first_part: bool,
        segs: &[Seg],
        limit: Option<gpui::Pixels>,
    ) {
        let end = part_end;
        // Тот же довод, что и в цикле: висеть пробелу можно только за
        // КРАЕМ, а при замере по максимальному содержимому края нет
        // (`pre-wrap-017`).
        let tail = if self.spaces_are_content() || (limit.is_none() && self.wrap.keep_spaces) {
            end
        } else {
            self.hang_tail(start, end)
        };
        let head = start + self.hang_first(start);
        let tail = tail - self.hang_last(tail, true, true);
        let (mut fl, mut fr) = self.flow_cut(out.len());
        // The same §9.5 shift for the last line (see the loop above).
        if let Some(w) = limit {
            let bare = self.span(segs, head, tail) - self.tail_spacing(tail);
            let ind0 = self.indent_of(head_of_part, first_part, limit);
            while (fl > 0.0 || fr > 0.0)
                && out.len() < 4096
                && f32::from(bare) > f32::from(w - ind0 - px(fl) - px(fr)) + 0.01
            {
                out.push(Line {
                    range: start..start,
                    width: px(0.),
                    ellipsis: false,
                    clamped: false,
                    vis_cut: None,
                    hyphen: false,
                    indent: ind0 + px(fl),
                });
                (fl, fr) = self.flow_cut(out.len());
            }
        }
        let indent = self.indent_of(head_of_part, first_part, limit) + px(fl);
        // Конец блока — тоже принудительный разрыв: хвост `pre-wrap`
        // последней строки висит условно (`pre-wrap-019`, `#test2`:
        // `"0 "` занимает 2ch, а не 1ch).
        let room = limit.map(|w| w - indent - px(fr));
        let bare = self.span(segs, head, tail) - self.tail_spacing(tail);
        out.push(Line {
            range: start..end,
            width: self.conditional_width(segs, head, end, bare, room),
            ellipsis: false,
            clamped: false,
            vis_cut: None,
            hyphen: false,
            indent,
        });
    }
}
