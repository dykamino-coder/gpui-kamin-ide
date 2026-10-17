//! Flat for painting; split out to keep the owning module within 250 lines.

mod rows;

use crate::text::paragraph::*;
use gpui::{App, Bounds, GlobalElementId, Hitbox, Pixels, Window, point, px, size};

impl Paragraph {
    pub(crate) fn paint_flat(
        &mut self,
        id: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let outer_nudge = window.replace_glyph_offset(self.glyph_nudge);
        let bounds = Bounds {
            origin: bounds.origin,
            size: size(bounds.size.width + self.width_nudge, bounds.size.height),
        };
        let segs = self.measure(window);
        if self.run_metrics.len() != self.runs.len() {
            self.run_metrics = self.measure_runs(window);
        }
        let count = self.lines.len();
        // Надбавки строк от сдвинутых кусков: шаг до следующей строки и
        // сдвиг набора внутри своей.
        let pads = self.line_padding();
        let step = |i: usize| -> Pixels {
            let (a, b) = pads.get(i).copied().unwrap_or((0.0, 0.0));
            self.line_height + px(a + b)
        };
        // Надбавка сверху опускает НАБОР строки: поднятый знак занимает её,
        // а базовая линия остаётся на своём месте относительно кегля.
        let above = |i: usize| -> Pixels { px(pads.get(i).copied().unwrap_or((0.0, 0.0)).0) };
        // Horizontal rows are published at prepaint (`ClampCut` reads them
        // in this frame's paint); the rotated frame keeps the paint-time path.
        if self.selection_vertical.is_some() || crate::text::vertical::in_rotated_frame() {
            self.publish_rows(bounds, count, &step);
        }
        let mut rev_off = self.reversed_offset(count, &step);
        let mut y = bounds.origin.y + px(rev_off);
        let runs = self.selection_runs(id, window);
        for (i, line) in self.lines.clone().into_iter().enumerate() {
            // Перевод строки в набор не отдаём: он уже сработал разрывом.
            let body = self.text[line.range.clone()].trim_end_matches('\n');
            let range = line.range.start..line.range.start + body.len();
            // Последняя строка абзаца и строка, оборванная жёстким разрывом,
            // по ширине не растягиваются: иначе абзац из одного слова разъехался
            // бы во всю колонку. Для них своя выключка (`text-align-last`).
            // Строка с СОХРАНЁННОЙ табуляцией не растягивается: позиции
            // табуляции обязаны совпасть с нерастянутой строкой
            // (css-text-4 §8.1, `text-align-justify-tabs-001`), а раздача
            // остатка их бы сдвинула.
            let align = self.line_align(i, &line);
            // Отступ первой строки занимает место В колонке: остаток на
            // выключку считается уже без него. Правый вырез обтекания
            // (`shape-outside`) — тоже: прижатая вправо строка упирается в
            // форму, а не в край коробки (circle-024: text-align right).
            let free_raw = bounds.size.width - line.width - line.indent - px(self.flow_cut(i).1);
            // Обрезание отрицательного остатка нужно только РАЗДАЧЕ
            // (`Justify`): растягивать переполненную строку нечем. Сдвиг по
            // `text-align` берёт остаток СО ЗНАКОМ — иначе широкая строка
            // всегда вылезает вправо, то есть по-ltr при любом письме
            // (см. `line_offset`).
            let free = if free_raw < px(0.) { px(0.) } else { free_raw };
            // Свисающий открывающий знак уходит ЗА край: строка сдвигается
            // влево на его ширину. Считается до выбора пути отрисовки —
            // выключенная строка свисает так же, как обычная.
            let hang = self.hang_first(line.range.start);
            let shift = self.span(&segs, line.range.start, line.range.start + hang);
            // Отступ первой строки идёт от НАЧАЛЬНОГО края (§16.1): в rtl это
            // правый край, и место ему уже отдано вычетом из остатка выше.
            // Прибавка слева считала бы его второй раз, а при выключке вправо
            // и вовсе гасила: `(W - w - indent) + indent = W - w`.
            let lead = if self.wrap.rtl { px(0.) } else { line.indent } - shift;
            // Строка с межсловным интервалом рисуется ПО СЛОВАМ: одним
            // набором промежутки не показать — шейпер о них не знает. Раздача
            // остатка при этом нулевая, слова просто встают по своим местам.
            // Межсловный интервал ставит слова по местам сам, поэтому строка
            // с ним рисуется тем же путём, что и выключенная. Интервал бывает
            // задан и НА КУСКЕ — тогда общего значения нет, а путь нужен тот
            // же (иначе `word-spacing` на `<span>` не действовал вовсе).
            // Строка с СОХРАНЁННОЙ табуляцией рисуется тоже по словам:
            // продвижение табуляции задаёт её позиция (`Seg::offset`), а
            // сплошной набор строки о ней не знает и кладёт глиф шрифта —
            // нарисованное выходило короче замеренного на целую позицию
            // (`text-align-justify-tabs-002`). Раздача остатка при этом
            // нулевая: растягивать такую строку нельзя (см. `no_stretch`),
            // слова просто встают по своим местам.
            if align == Align::Justify
                || body.contains('\u{9}')
                || (!self.joined_spacing()
                    && (self.word_spacing != px(0.)
                        || !self.word_spans.is_empty()
                        || self.letter_spans_diverge()))
                || !self.shift_spans.is_empty()
                || !self.rel_spans.is_empty()
                || !self.edge_spans.is_empty()
            {
                let (free, dx) = if align == Align::Justify {
                    (free, lead)
                } else {
                    let dx = line_offset(align, self.wrap.rtl, free_raw);
                    (px(0.), dx + lead)
                };
                // Набор строки опускается на её верхнюю надбавку: поднятый
                // кусок занимает добавленное место, а остальной текст
                // остаётся на своей базовой линии.
                self.paint_justified_line(
                    &line,
                    &range,
                    &segs,
                    free,
                    bounds,
                    y + above(i),
                    pads.get(i).copied().unwrap_or((0.0, 0.0)),
                    dx,
                    window,
                    cx,
                );
                if self.lines_reversed {
                    rev_off -= f32::from(step(i + 1));
                    y = bounds.origin.y + px(rev_off);
                } else {
                    y += step(i);
                }
                continue;
            }
            let dx = line_offset(align, self.wrap.rtl, free_raw) + lead;
            let at = point(bounds.origin.x + dx, y + above(i));
            let visible = if !self.wrap.keep_spaces {
                // Схлопываемый пробел конца строки УДАЛЯЕТСЯ (CSS 2.1 §16.6.1),
                // а не висит: рисовать его незачем, а подложка `<span>` под ним
                // вылезала за край коробки квадратом кегля (`c548-leadin-000`:
                // красный 25×25 справа от первой строки). Отличие от откаченной
                // правки у `trim_hanging`: там резалась ПОДЛОЖКА прогонов по
                // всему `hangs` (U+3000, U+2000..200A и пр.), здесь — только сам
                // отрезок набора, только U+0020/U+0009 и только при схлопывающем
                // `white-space`; прочие Zs-разделители висят как прежде.
                // CSS Text §4.1.3 keeps inline pre tails, including ::marker spacing.
                range.start..self.drop_collapsible_tail(range.start, range.end)
            } else {
                range.clone()
            };
            // CSS Text 3 phase II preserves non-collapsible tails, including
            // their inline background. UAX #9 L1 puts an RTL tail at visual
            // left; only its hanging advance lies outside the aligned line.
            // A conditionally hanging tail can still fit inside line.width.
            let hanging = if self.wrap.rtl {
                let tail = self.hang_tail(visible.start, visible.end);
                // CSS Text §8.2 gives hanging punctuation the same excluded
                // advance; line.width records whether it actually hung.
                let tail = tail - self.hang_last(tail, true, true).min(tail - visible.start);
                let full = self.span(&segs, visible.start, visible.end);
                (full - line.width)
                    .max(px(0.))
                    .min(self.span(&segs, tail, visible.end))
            } else {
                px(0.)
            };
            // Знак обрыва и знак переноса набираются вместе со строкой.
            let mark = if line.ellipsis && line.clamped && line.hyphen {
                // Обрыв на мягком переносе: знак переноса, затем многоточие.
                format!("{}{}", self.hyphen, self.line_mark(&line))
            } else if line.ellipsis {
                self.line_mark(&line).to_string()
            } else if line.hyphen {
                self.hyphen.to_string()
            } else {
                String::new()
            };
            if let Some(cut) = line.vis_cut.filter(|_| line.ellipsis) {
                // Строка целиком в видимом порядке, скрытые с конечного края
                // знаки отсекает маска; знак обрыва — сразу за видимой частью.
                let full = self.span(&segs, visible.start, visible.end);
                let (x0, mask_x, mark_x) = if self.wrap.rtl {
                    (at.x + line.width - full, at.x + line.width - cut, at.x)
                } else {
                    (at.x, at.x, at.x + cut)
                };
                let mask = Bounds {
                    origin: point(mask_x, bounds.origin.y - px(1000.)),
                    size: size(cut, bounds.size.height + px(2000.)),
                };
                let (base, exact) = window
                    .with_content_mask(Some(gpui::ContentMask { bounds: mask }), |window| {
                        self.paint_line(&visible, &runs, point(x0, at.y), "", window, cx)
                    });
                self.paint_suffix(
                    &mark,
                    line.range.start,
                    point(mark_x, at.y),
                    base,
                    exact,
                    window,
                    cx,
                );
            } else {
                self.paint_line(
                    &visible,
                    &runs,
                    point(at.x - hanging, at.y),
                    &mark,
                    window,
                    cx,
                );
            }
            if self.lines_reversed {
                rev_off -= f32::from(step(i + 1));
                y = bounds.origin.y + px(rev_off);
            } else {
                y += step(i);
            }
        }
        window.replace_glyph_offset(outer_nudge);
        for slot in self.atoms.iter_mut().filter(|s| !s.hidden) {
            slot.el.paint(window, cx);
        }
        for (i, (_, el, _)) in self.overlays.iter_mut().enumerate() {
            if !self.overlay_hidden.get(i).copied().unwrap_or(false) {
                el.paint(window, cx);
            }
        }
        if let (Some(global), Some(hitbox)) = (id, hitbox.clone()) {
            self.track_selection(global, &segs, bounds, hitbox, window);
        }
    }
}
