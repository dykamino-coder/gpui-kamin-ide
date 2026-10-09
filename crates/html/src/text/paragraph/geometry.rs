//! Геометрия строк: выравнивание, точка знака, коробка строчного блока, rtl-края.

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

    /// Где в коробке стоит байт текста: левый верхний угол его знака.
    pub(crate) fn point_of(&self, segs: &[Seg], at: usize, bounds: Bounds<Pixels>) -> Point<Pixels> {
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
            let free_raw =
                bounds.size.width - line.width - line.indent - px(self.flow_cut(row).1);
            let left = bounds.origin.x
                + line_offset(self.line_align(row, line), true, free_raw)
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
            let x = self.x_at(segs, at.clamp(line.range.start, line.range.end), Edge::Start) - from;
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
                let l = bounds.origin.x + line_offset(self.line_align(row, line), true, free) - shift
                    + px(self.flow_cut(row).0);
                let (lo, hi) = self.visual_extent_rtl(segs, a, b, line);
                (l + lo, l + hi)
            };
            let first_end = if row_s == row_e { end } else { self.lines[row_s].range.end };
            let last_start = if row_s == row_e { start } else { self.lines[row_e].range.start };
            let right = frag(row_s, start, first_end).1 + px(pad[1]);
            let left = (frag(row_e, last_start, end).0 - px(pad[3])).min(right);
            (left, right)
        } else {
            let left = x_in(row_s, start) - px(pad[3]);
            (left, (x_in(row_e, end) + px(pad[1])).max(left))
        };
        let top = bounds.origin.y + self.line_height * row_s as f32 - px(pad[0]);
        let bottom = (bounds.origin.y + self.line_height * (row_e + 1) as f32 + px(pad[2])).max(top);
        Some(Bounds {
            origin: point(left, top),
            size: gpui::size(right - left, bottom - top),
        })
    }

    /// Визуальный отрезок знаков `a..b` строки rtl-абзаца от её ЛЕВОГО края:
    /// прогоны UAX#9 в визуальном порядке (L2), как у `visual_x_rtl`, внутри
    /// rtl-прогона знаки идут справа налево. Пустой отрезок — точка `a`.
    pub(crate) fn visual_extent_rtl(&self, segs: &[Seg], a: usize, b: usize, line: &Line) -> (Pixels, Pixels) {
        let start = line.range.start;
        let end = start + trim_hanging(&self.text[line.range.clone()]);
        let a = a.clamp(start, end);
        let b = b.clamp(a, end);
        let info = unicode_bidi::BidiInfo::new(&self.text, Some(unicode_bidi::Level::rtl()));
        let Some(para) = info
            .paragraphs
            .iter()
            .find(|p| p.range.start <= start && start < p.range.end)
            .or_else(|| info.paragraphs.first())
        else {
            return (px(0.), px(0.));
        };
        if a == b {
            let x = self.visual_x_rtl(segs, a, line);
            return (x, x);
        }
        let (levels, runs) = info.visual_runs(para, start..end);
        let mut x = px(0.);
        let mut lo: Option<Pixels> = None;
        let mut hi: Option<Pixels> = None;
        for run in runs {
            let w = self.span(segs, run.start, run.end);
            let (s, e) = (a.max(run.start), b.min(run.end));
            if s < e {
                let rtl = levels.get(run.start).is_some_and(|l| l.is_rtl());
                let off = if rtl {
                    self.span(segs, e, run.end)
                } else {
                    self.span(segs, run.start, s)
                };
                let part = self.span(segs, s, e);
                lo = Some(lo.map_or(x + off, |v: Pixels| v.min(x + off)));
                hi = Some(hi.map_or(x + off + part, |v: Pixels| v.max(x + off + part)));
            }
            x += w;
        }
        (lo.unwrap_or(px(0.)), hi.unwrap_or(px(0.)))
    }

    /// Визуальное продвижение места `at` от ЛЕВОГО края rtl-строки.
    ///
    /// На место куска ставится нейтральный U+FFFC (так UAX#9 видит
    /// замещаемый объект), строка разбирается с базой rtl, и прогоны идут в
    /// ВИЗУАЛЬНОМ порядке (L2), как у отрисовки (`paint_line`): слева
    /// складываются ширины прогонов до прогона метки, внутри него — знаки
    /// до метки (ltr-прогон) или после неё (rtl-прогон). Пример
    /// `abs-pos-non-replaced-vrl-008`: строка «34» + абсолют — метка уровня 1
    /// после числа уровня 2 встаёт ЛЕВЕЕ числа, и коробка висит от левого
    /// края строки, а не от правого.
    pub(crate) fn visual_x_rtl(&self, segs: &[Seg], at: usize, line: &Line) -> Pixels {
        let start = line.range.start;
        let end = start + trim_hanging(&self.text[line.range.clone()]);
        let at = at.clamp(start, end);
        const MARK: usize = 3; // U+FFFC в UTF-8
        let mut probe = String::with_capacity(self.text.len() + MARK);
        probe.push_str(&self.text[..at]);
        probe.push('\u{fffc}');
        probe.push_str(&self.text[at..]);
        let info = unicode_bidi::BidiInfo::new(&probe, Some(unicode_bidi::Level::rtl()));
        let Some(para) = info
            .paragraphs
            .iter()
            .find(|p| p.range.start <= start && start < p.range.end)
            .or_else(|| info.paragraphs.first())
        else {
            return px(0.);
        };
        let (levels, runs) = info.visual_runs(para, start..end + MARK);
        // Отрезок метки-строки обратно в отрезок исходного текста.
        let orig = |p: usize| if p <= at { p } else { p - MARK };
        let width = |a: usize, b: usize| self.span(segs, orig(a), orig(b));
        let mut x = px(0.);
        for run in runs {
            let mark_in = run.start <= at && at < run.end;
            if !mark_in {
                x += width(run.start, run.end);
                continue;
            }
            let rtl = levels.get(run.start).is_some_and(|l| l.is_rtl());
            x += if rtl {
                width(at + MARK, run.end)
            } else {
                width(run.start, at)
            };
            break;
        }
        x
    }

    /// Уровень bidi у места куска вне потока — справа налево ли? Сам кусок
    /// в тексте знака не имеет, поэтому на его место ставится нейтральный
    /// U+FFFC (так UAX#9 видит замещаемый объект): его уровень решают
    /// соседи по правилам N1/N2.
    pub(crate) fn rtl_level_at(&self, at: usize) -> bool {
        let at = at.min(self.text.len());
        let mut probe = String::with_capacity(self.text.len() + 3);
        probe.push_str(&self.text[..at]);
        probe.push('\u{fffc}');
        probe.push_str(&self.text[at..]);
        let base = if self.wrap.rtl {
            unicode_bidi::Level::rtl()
        } else {
            unicode_bidi::Level::ltr()
        };
        let forced = if self.plaintext.is_some() { None } else { Some(base) };
        let info = unicode_bidi::BidiInfo::new(&probe, forced);
        info.levels.get(at).map_or(self.wrap.rtl, |l| l.is_rtl())
    }

    /// Статическая позиция БЛОЧНОГО куска вне потока: строчное начало —
    /// край содержимого (без `text-indent`: отступ — свойство первой
    /// СТРОКИ, а гипотетическая коробка — блок), блочное — начало строки,
    /// следующей за той, где кусок стоит в тексте (CSS 2.1 §10.6.4 «if
    /// position had been static»: блок в строчном содержимом рвёт строку и
    /// встаёт после неё). Кусок в самом начале строки ничего перед собой не
    /// имеет — строка рвётся ДО него, и место — верх этой же строки.
    /// Порядок строк на экране при `lines_reversed` зеркален (см. `point_of`).
    pub(crate) fn next_line_point(&self, at: usize, bounds: Bounds<Pixels>) -> Point<Pixels> {
        let count = self.lines.len();
        let row = self
            .lines
            .iter()
            .position(|l| at < l.range.end)
            .unwrap_or(count.saturating_sub(1));
        // Есть ли перед куском в его строке настоящее содержимое (служебные
        // распорки `SPACER`/`ZWSP` места не занимают).
        let after = self.lines.get(row).is_some_and(|l| {
            let end = at.clamp(l.range.start, l.range.end);
            self.text
                .get(l.range.start..end)
                .is_some_and(|t| t.chars().any(|c| !matches!(c, '\u{feff}' | '\u{200b}')))
        });
        let visual = if self.lines_reversed {
            count.saturating_sub(1).saturating_sub(row) as f32
        } else {
            row as f32
        };
        // Следующая строка: при обратном порядке она ВЫШЕ на экране.
        let step = match (after, self.lines_reversed) {
            (false, _) => 0.0,
            (true, false) => 1.0,
            (true, true) => -1.0,
        };
        point(bounds.origin.x, bounds.origin.y + self.line_height * (visual + step))
    }
}

/// Правила переноса из стиля — и признак, нужна ли своя раскладка вовсе.
///
/// Пока своя раскладка не умеет выделение мышью, поэтому обычный текст
/// остаётся на выделяемом элементе движка. Сюда уходит только то, что иначе
/// не выразить.
/// Правила переноса из стиля.
///
/// Своя раскладка считает ВЕСЬ текст: перенос, выключка и свисающая
/// пунктуация должны решаться одним алгоритмом, иначе соседние абзацы одной
/// страницы ломаются по-разному. Поэтому правила есть всегда — отбор «кому
/// своя раскладка нужна, а кому нет» отсюда снят.
pub fn rules(c: &crate::style::computed::Computed) -> Option<Wrap> {
    Some(wrap_of(c))
}

/// Сдвиг строки вдоль коробки по `text-align` — с УЧЁТОМ ЗНАКА остатка.
///
/// Дословный перенос Blink `length_utils.cc:1607 LineOffsetForTextAlign`.
/// Смысл в том, что обрезание отрицательного остатка зависит от СТОРОНЫ
/// ПИСЬМА БЛОКА, а не от значения `text-align`:
///
/// * ltr — отрицательный остаток гасится всегда: «Wide lines spill out of the
///   block based off direction. So even if text-align is right, if direction
///   is LTR, wide lines should overflow out of the right side of the block»
///   (`length_utils.cc:1634-1636`);
/// * rtl — не гасится никогда: «The direction of the block should determine
///   what happens with wide lines. In particular with RTL blocks, wide lines
///   should still spill out to the left» (`length_utils.cc:1620-1622`).
///
/// По спеке это css-text-4 §7.1 (`right` — «Inline-level content is aligned to
/// the line-right edge of the line box», без оговорки на переполнение) вместе
/// с CSS 2.1 §16.2 (начальное значение `text-align` в rtl действует как
/// `right`) и §9.4.2 («then the inline box overflows the line box»).
///
/// `Justify` сюда не заходит: раздача остатка идёт своим путём и берёт
/// остаток УЖЕ обрезанным — растягивать переполненную строку нечем.
pub(crate) fn line_offset(align: Align, rtl: bool, free: Pixels) -> Pixels {
    let zero = px(0.);
    match align {
        Align::Right if rtl => free,
        Align::Right => free.max(zero),
        Align::Left if rtl => free.min(zero),
        Align::Left => zero,
        // При rtl и положительном остатке — та же половина, что и при ltr;
        // при отрицательном строка держится правого края целиком.
        Align::Center if rtl && free <= zero => free,
        Align::Center => (free / 2.).max(zero),
        Align::Justify => zero,
    }
}

/// Выключка из стиля.
pub fn align_of(a: Option<crate::style::computed::TextAlign>) -> Align {
    a.map(align_of_value).unwrap_or(Align::Left)
}

/// Выключка абзаца с разворотом логических краёв по стороне письма.
pub fn align_for(c: &crate::style::computed::Computed) -> Align {
    let rtl = c.rtl == Some(true);
    let value = c
        .text_align
        .unwrap_or(crate::style::computed::TextAlign::Start)
        .physical(rtl);
    let align = align_of_value(value);
    // `text-justify: none` — растягивать запрещено, и строка идёт к началу:
    // у письма справа налево началом служит правый край.
    if align == Align::Justify && c.no_justify == Some(true) {
        return if rtl { Align::Right } else { Align::Left };
    }
    align
}

/// Выключка из заданного значения.
pub fn align_of_value(a: crate::style::computed::TextAlign) -> Align {
    match a {
        crate::style::computed::TextAlign::Center => Align::Center,
        crate::style::computed::TextAlign::Right => Align::Right,
        crate::style::computed::TextAlign::Justify => Align::Justify,
        _ => Align::Left,
    }
}
