//! Layout lines for breaking; split out to keep the owning module within 250 lines.

mod opportunities;

use crate::text::paragraph::*;
use gpui::{Pixels, px};

impl Paragraph {
    /// То же для ЧАСТИ текста: выравнивание длин идёт по группам между
    /// жёсткими разрывами, и каждая группа набирается своей ширины.
    pub(crate) fn lay_in(
        &self,
        part: std::ops::Range<usize>,
        limit: Option<Pixels>,
        segs: &[Seg],
    ) -> Vec<Line> {
        let x = |i: usize| -> Pixels { self.x_at(segs, i, Edge::End) };
        let mut out: Vec<Line> = Vec::new();
        // Начало строки: схлопываемые пробелы после переноса не рисуются и в
        // ширину не входят. При сохранённых пробелах (`pre*`) они значимы.
        // Правило берётся В ЭТОМ МЕСТЕ, а не у абзаца целиком: `white-space`
        // на вложенном `<span>`/`display: inline` действует на свои знаки, и
        // абзац об этом не знает. Пока смотрели правило абзаца, сохранённые
        // пробелы вложенного куска исчезали с начала перенесённой строки
        // (`ws-break-spaces-applies-to-001`).
        let bol = |at: usize| -> usize {
            if self.wrap_at(at).keep_spaces {
                at
            } else {
                at + skip_leading(&self.text[at..])
            }
        };
        let mut start = bol(part.start);
        // Отступ первой строки (`text-indent`) — свойство СТРОКИ, а не абзаца:
        // его получает первая строка блока, при `each-line` — первая после
        // каждого жёсткого разрыва, при `hanging` — все остальные. Поэтому
        // здесь ведётся, начинает ли строка кусок и первый ли это кусок блока:
        // группы между жёсткими разрывами набираются и по отдельности
        // (выравнивание длин), и подряд в одном проходе.
        let mut head_of_part = true;
        let mut first_part = part.start == 0;
        let mut last_fit: Option<usize> = None;
        let opportunities = self.part_opportunities(&part);
        let mut i = 0usize;
        while i < opportunities.len() {
            let Stop { at, mandatory } = opportunities[i];
            if at <= start {
                i += 1;
                continue;
            }
            // Отступ отбирает место у СВОЕЙ строки: на неё остаётся уже
            // меньшая ширина, а отрицательный отступ, наоборот, добавляет.
            let ind = self.indent_of(head_of_part, first_part, limit);
            // Вырез обтекания сужает СВОЮ строку: левый входит в отступ
            // строки, правый просто отбирает ширину (css-shapes-1 §2).
            let (fl, fr) = self.flow_cut(out.len());
            let ind = ind + px(fl);
            let limit = limit.map(|w| w - ind - px(fr));
            let (head, width, over) = self.candidate_width(start, at, part.end, segs, limit);
            // Обязательный разрыв проверяется ПОСЛЕ переполнения: до него
            // строка может не влезать, и тогда сперва переносится она.
            // Раньше кусок перед переводом строки уходил в строку целиком,
            // сколько бы ни переполнял коробку (`pre-wrap-leading-spaces`).
            if mandatory && !over {
                // Хвост `pre-wrap` перед принудительным разрывом висит
                // УСЛОВНО: влезшая часть занимает место (css-text-3 §4.1.3).
                let width = self.conditional_width(segs, head, at, width, limit);
                out.push(Line {
                    range: start..at,
                    width,
                    ellipsis: false,
                    clamped: false,
                    vis_cut: None,
                    hyphen: false,
                    indent: ind,
                });
                start = bol(at);
                // За жёстким разрывом начинается новый кусок: при `each-line`
                // отступ повторяется, но «первым куском блока» он уже не будет.
                head_of_part = true;
                first_part = false;
                last_fit = None;
                i += 1;
                continue;
            }
            // CSS 2.1 §9.5: a line box shortened by floats (here their
            // `shape-outside` cut) too small for any content moves down until
            // some content fits or the floats end (spec-examples
            // `shape-outside-001`: the last word skips the V's tip line).
            if over
                && last_fit.filter(|c| *c > start).is_none()
                && (fl > 0.0 || fr > 0.0)
                && out.len() < 4096
            {
                out.push(Line {
                    range: start..start,
                    width: px(0.),
                    ellipsis: false,
                    clamped: false,
                    vis_cut: None,
                    hyphen: false,
                    indent: ind,
                });
                continue;
            }
            if over {
                // Переносим по последней подошедшей точке; если её нет —
                // рвём по знакам, но только когда это разрешено.
                let cut = last_fit.filter(|c| *c > start).unwrap_or_else(|| {
                    // Разрешение рвать слово берётся ПО МЕСТУ переполнения:
                    // `overflow-wrap` на вложенном `<span>` действует только
                    // на его знаки.
                    // Разрешение берётся ПО МЕСТУ, где строка переполнилась,
                    // а не по её началу: `overflow-wrap` на вложенном
                    // `<span>` действует на свои знаки, и кусок этот обычно
                    // начинается посреди строки
                    // (`overflow-wrap-anywhere-inline-*`).
                    // `white-space: nowrap` запрещает и аварийный разрыв:
                    // `overflow-wrap` действует, только когда перенос вообще
                    // разрешён (`overflow-wrap-002`).
                    // …и ВНУТРИ строки тоже: `<span>` с `overflow-wrap:
                    // anywhere` посреди неразрывного ряда не касается ни его
                    // начала, ни конца (`overflow-wrap-anywhere-inline-002/004`:
                    // ряд «X<span>XX</span>XX» уходил одной строкой за край).
                    if self.emergency_ok(start)
                        || self.emergency_ok(at.saturating_sub(1))
                        || self.emergency_inside(start, at)
                    {
                        self.cut_by_char(start, at, limit, &x)
                    } else {
                        at
                    }
                });
                let tail = if self.spaces_are_content() {
                    cut
                } else {
                    self.hang_tail(start, cut)
                };
                let tail = tail - self.hang_last(tail, false, true);
                // Разрыв по мягкому переносу: на строке остаётся знак
                // переноса, и он же входит в её ширину.
                let hyphen = self.text[..cut].ends_with('\u{00ad}');
                let extra = if hyphen {
                    self.hyphen_width(cut)
                } else {
                    px(0.)
                };
                out.push(Line {
                    range: start..self.drop_collapsible_tail(start, cut),
                    width: self.span(segs, head, tail) - self.tail_spacing(tail) + extra,
                    ellipsis: false,
                    clamped: false,
                    vis_cut: None,
                    hyphen,
                    indent: ind,
                });
                start = bol(cut);
                // Мягкий перенос кусок не кончает: следующая строка отступа
                // не получает (кроме `hanging`, где его получают именно они).
                head_of_part = false;
                last_fit = None;
                // Ту же точку проверяем заново от нового начала строки: за
                // одним переносом может идти следующий.
                continue;
            }
            last_fit = Some(at);
            i += 1;
        }
        if start < part.end || out.is_empty() {
            self.push_final_line(
                &mut out,
                start,
                part.end,
                head_of_part,
                first_part,
                segs,
                limit,
            );
        }
        out
    }
}
