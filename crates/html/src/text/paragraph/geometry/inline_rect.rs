//! Inline rect for geometry; split out to keep the owning module within 250 lines.

use super::{align_of_value, line_offset};
use crate::text::paragraph::*;
use gpui::{Bounds, Pixels, Point, point, px};

impl Paragraph {
    /// Выключка строки `i`: последняя строка и строка перед жёстким разрывом
    /// идут своей выключкой (`text-align-last`), `plaintext` решает сторону
    /// по абзацу между разрывами. Общая для отрисовки и для мест атомов.
    pub(crate) fn line_align(&self, i: usize, line: &Line) -> Align {
        let count = self.lines.len();
        let body = self.text[line.range.clone()].trim_end_matches('\n');
        let last_line = i + 1 == count || body.len() < line.range.len();
        // Строка с СОХРАНЁННОЙ табуляцией не растягивается (позиции
        // табуляции обязаны совпасть с нерастянутой строкой), но выключку
        // ПОСЛЕДНЕЙ строки (`text-align-last`) она не получает: к
        // табуляции та отношения не имеет.
        let no_stretch = last_line;
        // При `plaintext` сторона письма своя у каждого АБЗАЦА между
        // жёсткими разрывами (не у строки: мягкий перенос сторону не
        // меняет). От неё же зависят `start` и `end`.
        let own_align =
            match self.plaintext {
                Some(logical) => {
                    let start = self.text[..line.range.start]
                        .rfind('\n')
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    let end = self.text[start..]
                        .find('\n')
                        .map(|i| start + i)
                        .unwrap_or(self.text.len());
                    // При `unicode-bidi: plaintext` сторона КАЖДОГО абзаца
                    // берётся по первому сильному знаку (css-writing-modes-4
                    // §2.2 -> UAX#9 P2/P3), а не у элемента. Порядок глифов это
                    // уже учитывал (`BidiInfo::new(text, None)`), выключка —
                    // нет. Нейтральный абзац сильного знака не имеет и остаётся
                    // на стороне элемента.
                    align_of_value(logical.physical(
                        first_strong_rtl(&self.text[start..end]).unwrap_or(self.wrap.rtl),
                    ))
                }
                None => self.align,
            };
        // Нерастянутая выключка: `justify` прижимает строку к НАЧАЛУ, а
        // начало у письма справа налево — правый край, не левый.
        let flat = |a: Align| match a {
            Align::Justify if self.wrap.rtl => Align::Right,
            Align::Justify => Align::Left,
            other => other,
        };
        let align = if last_line {
            self.align_last.unwrap_or(flat(own_align))
        } else if no_stretch {
            flat(own_align)
        } else {
            own_align
        };
        self.ruby_line_align(align, &line.range)
    }
}

impl Paragraph {
    /// Где в коробке стоит байт текста: левый верхний угол его знака.
    pub(crate) fn point_of(
        &self,
        segs: &[Seg],
        at: usize,
        bounds: Bounds<Pixels>,
    ) -> Point<Pixels> {
        let row = self
            .lines
            .iter()
            .position(|l| at < l.range.end)
            .unwrap_or(self.lines.len().saturating_sub(1));
        let Some(line) = self.lines.get(row) else {
            return bounds.origin;
        };
        let from = self.x_at(segs, line.range.start, Edge::Start);
        let x = self.x_at(segs, at.max(line.range.start), Edge::Start) - from;
        // Стартовое смещение строки — как у отрисовки: отступ первой строки,
        // свисающий открывающий знак, левый вырез обтекания. Без него точка
        // жила от голого края коробки, и статическая позиция абсолюта в
        // строке с `text-indent` промахивалась ровно на отступ
        // (htb-ltr-*: регресс 08-12, зелёные квадраты не закрывали красное).
        let hang = self.hang_first(line.range.start);
        let shift = self.span(segs, line.range.start, line.range.start + hang);
        let lead =
            if self.wrap.rtl { px(0.) } else { line.indent } - shift + px(self.flow_cut(row).0);
        // Повёрнутый абзац при `direction: rtl`: место считается ВИЗУАЛЬНО
        // (`visual_x_rtl`) — строка прижата к правому краю до-поворотной
        // коробки и переставлена разбором UAX#9, а логическое продвижение от
        // левого края верно только для одного rtl-прогона.
        if self.wrap.rtl && crate::text::vertical::in_rotated_frame() {
            let free_raw = bounds.size.width - line.width - line.indent - px(self.flow_cut(row).1);
            let left = bounds.origin.x + line_offset(self.line_align(row, line), true, free_raw)
                - shift
                + px(self.flow_cut(row).0);
            let visual = if self.lines_reversed {
                self.lines.len().saturating_sub(1).saturating_sub(row)
            } else {
                row
            };
            return point(
                left + self.visual_x_rtl(segs, at, line),
                bounds.origin.y + self.line_height * visual as f32,
            );
        }
        // ЗАМЕРЕНО И ОТКАЧЕНО (04.09): прибавлять сюда долю ВЫКЛЮЧКИ
        // (`text-align: center|right`) тем же счётом, что и отрисовка
        // (`free/2` и `free`). Срез из 633 пар статической позиции и
        // абсолютов: 382 -> 359, приобретено 0, потеряно 23 — вся семья
        // `abs-pos-non-replaced-v{lr,rl}-1xx` (0.00 -> 2.67) и
        // `abspos-width-change-inline-container-001`. В повёрнутом абзаце
        // `bounds.size.width` — не та ось, и остаток строки считается не от
        // той стороны; возвращать вместе с осевым остатком.
        // Строки рисуются снизу вверх (`lines_reversed` — это `vertical-lr`),
        // и НОМЕР строки в списке тогда зеркален её месту на экране. Щуп
        // статической позиции брал номер как есть и садился на зеркальную
        // строку — оттого вся семья `abs-pos-non-replaced-vlr-*` промахивалась
        // ровно на отражение, а `-vrl-*` (там порядок прямой) была цела.
        let visual = if self.lines_reversed {
            self.lines.len().saturating_sub(1).saturating_sub(row)
        } else {
            row
        };
        point(
            bounds.origin.x + lead + x,
            bounds.origin.y + self.line_height * visual as f32,
        )
    }
}

impl Paragraph {
    /// Содержащий блок из фрагментов строчной коробки с содержимым
    /// `start..end` (CSS 2.1 §10.1 п.4.1; Blink `out_of_flow_layout_part.cc`
    /// `ComputeInlineContainingBlocks`): левый верхний угол — начало первого
    /// фрагмента, правый нижний — конец последнего непустого, размер не
    /// меньше нуля. Края — по отбивке (`pad`). Только горизонтальный ltr-абзац
    /// с прямым порядком строк; иначе `None`.
    pub(crate) fn inline_cb_rect(
        &self,
        segs: &[Seg],
        start: usize,
        end: usize,
        pad: [f32; 4],
        bounds: Bounds<Pixels>,
    ) -> Option<Bounds<Pixels>> {
        if self.lines_reversed || self.lines.is_empty() {
            return None;
        }
        let bytes = self.text.as_bytes();
        let blank = |b: u8| matches!(b, b' ' | b'\n' | b'\t');
        let end = end.min(self.text.len());
        let mut start = start.min(end);
        let mut end = end;
        // Схлопнутые пробелы у края строки фрагмента не дают (css-text-3
        // §4.1.2: пробел в конце строки снимается, в начале — тоже).
        while end > start && blank(bytes[end - 1]) {
            end -= 1;
        }
        while start < end && blank(bytes[start]) {
            start += 1;
        }
        let row_of = |at: usize| {
            self.lines
                .iter()
                .position(|l| at < l.range.end)
                .unwrap_or(self.lines.len() - 1)
        };
        let row_s = row_of(start);
        let row_e = if end > start { row_of(end - 1) } else { row_s };
        let x_in = |row: usize, at: usize| -> Pixels {
            let line = &self.lines[row];
            let from = self.x_at(segs, line.range.start, Edge::Start);
            let x = self.x_at(
                segs,
                at.clamp(line.range.start, line.range.end),
                Edge::Start,
            ) - from;
            let hang = self.hang_first(line.range.start);
            let shift = self.span(segs, line.range.start, line.range.start + hang);
            bounds.origin.x + line.indent - shift + px(self.flow_cut(row).0) + x
        };
        let (left, right) = if self.wrap.rtl {
            // Письмо справа налево: начало фрагмента — его ПРАВЫЙ край,
            // конец — левый; прямоугольник фрагмента строки — крайние
            // визуальные места его краёв (`visual_x_rtl`, разбор UAX#9 как у
            // отрисовки), строка прижата по `line_offset`.
            let frag = |row: usize, a: usize, b: usize| -> (Pixels, Pixels) {
                let line = &self.lines[row];
                let free = bounds.size.width - line.width - line.indent - px(self.flow_cut(row).1);
                let hang = self.hang_first(line.range.start);
                let shift = self.span(segs, line.range.start, line.range.start + hang);
                let l = bounds.origin.x + line_offset(self.line_align(row, line), true, free)
                    - shift
                    + px(self.flow_cut(row).0);
                let (lo, hi) = self.visual_extent_rtl(segs, a, b, line);
                (l + lo, l + hi)
            };
            let first_end = if row_s == row_e {
                end
            } else {
                self.lines[row_s].range.end
            };
            let last_start = if row_s == row_e {
                start
            } else {
                self.lines[row_e].range.start
            };
            let right = frag(row_s, start, first_end).1 + px(pad[1]);
            let left = (frag(row_e, last_start, end).0 - px(pad[3])).min(right);
            (left, right)
        } else {
            let left = x_in(row_s, start) - px(pad[3]);
            (left, (x_in(row_e, end) + px(pad[1])).max(left))
        };
        let top = bounds.origin.y + self.line_height * row_s as f32 - px(pad[0]);
        let bottom =
            (bounds.origin.y + self.line_height * (row_e + 1) as f32 + px(pad[2])).max(top);
        Some(Bounds {
            origin: point(left, top),
            size: gpui::size(right - left, bottom - top),
        })
    }
}
