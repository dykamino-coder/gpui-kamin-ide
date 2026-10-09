//! Разбиение на строки: split, lay_in, балансировка, аварийные разрывы, места переноса (UAX #14).

use crate::text::paragraph::*;
use gpui::{Pixels, Window, px};

impl Paragraph {
    /// Разбить текст на строки под заданную ширину и оборвать по `line-clamp`.
    ///
    /// Порядок важен: сперва обрыв, потом выравнивание длин. Выровнять надо
    /// то, что ОСТАЛОСЬ видимым, и с учётом места, отнятого многоточием
    /// (`text-wrap-balance-line-clamp-003`).
    /// Разрез с памятью: раскладка гоняет его по 3-5 раз на абзац за кадр
    /// (min/max/definite у гибкого родителя + подготовка), а разрез — самое
    /// дорогое место резчика. Ключ обязан покрывать ВСЁ, что читает
    /// `split_uncached`, иначе устаревшие переносы сдвинут пиксели.
    pub(crate) fn split(&self, limit: Option<Pixels>, window: &mut Window) -> Vec<Line> {
        self.prepare_hyphen_widths(window);
        // Подбор кегля мутирует абзац между вызовами — ключ это видит
        // (font_size в ключе замера).
        let key = self.split_key(limit);
        if let Some(hit) = SPLITS.with(|c| c.borrow().get(&key).cloned()) {
            return (*hit).clone();
        }
        let lines = self.split_uncached(limit, window);
        SPLITS.with(|c| {
            let mut m = c.borrow_mut();
            // Прямолинейный сброс при переполнении: страница с тысячами
            // абзацев дороже промахов одного сброса.
            if m.len() >= 2048 {
                m.clear();
            }
            m.insert(key, std::rc::Rc::new(lines.clone()));
        });
        lines
    }

    pub(crate) fn split_uncached(&self, limit: Option<Pixels>, window: &mut Window) -> Vec<Line> {
        let segs = self.measure(window);
        let mut lines = self.lay(limit, &segs);
        // Обрезка строк контейнером с `text-overflow: ellipsis`: не влезшая
        // строка усекается с многоточием (css-overflow-3 §text-overflow).
        if self.text_overflow
            && let Some(limit) = limit
        {
            // Строка, на которую сядет знак обрыва `line-clamp`, усекается
            // им самим (css-overflow-4 §block-ellipsis: место отбирается «as
            // if wrapping» до точки переноса, а не посимвольно): непереносимое
            // слово уходит целиком, и остаётся одно «…»
            // (`webkit-line-clamp-036`, `line-clamp-auto-009`).
            let clamp_line = self
                .clamp
                .filter(|n| *n > 0 && (lines.len() > *n || self.clamp_force))
                .map(|n| n.min(lines.len()).saturating_sub(1));
            for (i, line) in lines.iter_mut().enumerate() {
                if Some(i) == clamp_line {
                    continue;
                }
                if line.width > limit + px(0.5) && !line.ellipsis {
                    self.ellipsize(line, limit, &segs, window);
                }
            }
        }
        let cut = self
            .clamp
            .filter(|n| *n > 0 && lines.len() > *n)
            .zip(limit)
            .filter(|_| self.wrap.balance);
        let Some((max, limit)) = cut else {
            return self.clamp_lines(self.balanced(lines, limit, &segs), limit, window);
        };
        // Видимый текст — тот, что уместился в обрезанные строки. Его и
        // раскладываем заново, ища самую узкую колонку, в которой он всё ещё
        // помещается в те же строки.
        let end = self
            .clamp_lines(lines, Some(limit), window)
            .last()
            .map(|l| l.range.end)
            .unwrap_or(0);
        // ЗАМЕРЕНО И ОТКАЧЕНО: резервировать при подборе место под
        // МНОГОТОЧИЕ (условие `l.width + ell <= middle`). Срез из 27 пар
        // семей balance/clamp/text-wrap: 0 и 0 — колонка, к которой сходится
        // двоичный поиск, от этого условия не меняется.
        // `text-wrap-balance-line-clamp-*` держит не подбор ширины.
        // Место под МНОГОТОЧИЕ входит в колонку: на оборванной строке за
        // текстом рисуется знак обрыва, и колонка, в которую он не влезает,
        // подбором не годится (css-text-4 §5: обрыв — часть последней
        // строки). Прошлый заход мерил ширину строки КАК ЕСТЬ; здесь хвост
        // сперва обрезается, как это делает сам обрыв (`ellipsize`).
        let ell = self.suffix_width(self.clamp_str(), 0, window);
        let (mut narrow, mut wide) = (px(0.), limit);
        for _ in 0..12 {
            let middle = (narrow + wide) / 2.;
            let probe = self.lay(Some(middle), &segs);
            let fits = probe.get(max - 1).is_some_and(|l: &Line| {
                if l.range.end < end {
                    return false;
                }
                let cut = l.range.start + trim_hanging(&self.text[l.range.clone()]);
                self.span(&segs, l.range.start, cut) + ell <= middle
            });
            if fits {
                wide = middle;
            } else {
                narrow = middle;
            }
        }
        self.clamp_lines(self.lay(Some(wide), &segs), Some(limit), window)
    }

    /// `text-wrap: balance` — те же строки, но одной длины.
    pub(crate) fn balanced(&self, lines: Vec<Line>, limit: Option<Pixels>, segs: &[Seg]) -> Vec<Line> {
        let Some(limit) = limit else { return lines };
        if !self.wrap.balance || lines.len() < 2 {
            return lines;
        }
        // Группы строк, разделённые ЖЁСТКИМ разрывом, выравниваются по
        // отдельности (css-text-4 §7.1): у каждой своя ширина, одной на весь
        // абзац не хватает (`text-wrap-balance-004`).
        let mut out: Vec<Line> = Vec::new();
        let mut i = 0usize;
        while i < lines.len() {
            let last = lines[i..]
                .iter()
                .position(|l| self.text[l.range.clone()].ends_with('\n'))
                .map(|k| i + k)
                .unwrap_or(lines.len() - 1);
            let part = lines[i].range.start..lines[last].range.end;
            out.extend(self.balanced_part(part, last + 1 - i, limit, segs));
            i = last + 1;
        }
        out
    }

    /// Выравнивание длин ОДНОЙ группы строк: поиск самой узкой колонки, в
    /// которой строк не прибавилось. Тогда последняя строка перестаёт быть
    /// коротким огрызком.
    pub(crate) fn balanced_part(
        &self,
        part: std::ops::Range<usize>,
        target: usize,
        limit: Pixels,
        segs: &[Seg],
    ) -> Vec<Line> {
        if target < 2 {
            return self.lay_in(part, Some(limit), segs);
        }
        let (mut narrow, mut wide) = (px(0.), limit);
        for _ in 0..12 {
            let middle = (narrow + wide) / 2.;
            if self.lay_in(part.clone(), Some(middle), segs).len() <= target {
                wide = middle;
            } else {
                narrow = middle;
            }
        }
        self.lay_in(part, Some(wide), segs)
    }

    /// Набор строк под заданную ширину — без выравнивания их длин.
    pub(crate) fn lay(&self, limit: Option<Pixels>, segs: &[Seg]) -> Vec<Line> {
        self.lay_in(0..self.text.len(), limit, segs)
    }

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
            // Хвостовые пробелы висят за краем: в ширину строки они не входят.
            // Хвостовые пробелы висят за краем СТРОКИ — то есть когда край
            // вообще есть. При замере по максимальному содержимому предела
            // нет, и сохранённый пробел в ширину ВХОДИТ (`pre-wrap-017`:
            // коробка `width: max-content` выходила на знак уже).
            let measured =
                if self.spaces_are_content() || (limit.is_none() && self.wrap.keep_spaces) {
                    at
                } else {
                    self.hang_tail(start, at)
                };
            // Свисающее за края в ширину строки не входит — ни открывающий
            // знак в начале, ни точка с запятой в конце.
            let head = start + self.hang_first(start);
            // Свисает ли знак — зависит от того, влезает ли строка БЕЗ него;
            // поэтому ширина считается дважды: сначала без свисания.
            let bare = self.span(&segs, head, measured);
            let tight = limit.is_some_and(|w| bare > w);
            let tail_hang = self.hang_last(measured, at >= part.end, tight);
            let mut width = self.span(&segs, head, measured - tail_hang)
                - self.tail_spacing(measured - tail_hang);
            // Строка, кончающаяся мягким переносом, несёт ещё и знак переноса.
            if self.text[..measured].ends_with('\u{00ad}') {
                width += self.hyphen_width(measured);
            }
            // Допуск в сотую точки: ширина строки складывается из замеров
            // кусков и знака переноса, и на ТОЧНОМ совпадении с коробкой
            // накопленная ошибка решала исход (`hyphens-manual-011`: строка,
            // влезающая ровно, уходила на перенос).
            let over = limit.is_some_and(|w| f32::from(width) > f32::from(w) + 0.01);
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
                let extra = if hyphen { self.hyphen_width(cut) } else { px(0.) };
                out.push(Line {
                    range: start..self.drop_collapsible_tail(start, cut),
                    width: self.span(&segs, head, tail) - self.tail_spacing(tail) + extra,
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
            let end = part.end;
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
                let bare = self.span(&segs, head, tail) - self.tail_spacing(tail);
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
            let bare = self.span(&segs, head, tail) - self.tail_spacing(tail);
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
        out
    }

    /// Ширина строки перед ПРИНУДИТЕЛЬНЫМ разрывом (конец блока — тоже он) с
    /// учётом условного висения, css-text-3 §4.1.3 шаг 4: «If white-space is
    /// set to pre-wrap, the UA must (unconditionally) hang this sequence,
    /// unless the sequence is followed by a forced line break, in which case
    /// it must conditionally hang the sequence instead». Условно висящее
    /// входит в ширину, пока влезает. Висящие без условий знаки перед ним
    /// (U+3000 при `normal`) висят, только если условный ряд начинается уже
    /// НЕ раньше края (`hanging-whitespace-003`: строки с рядом от 6, 5 и 4ch
    /// в коробке 4ch висят целиком, ряд от 3ch занимает место).
    pub(crate) fn conditional_width(
        &self,
        segs: &[Seg],
        head: usize,
        end: usize,
        bare: Pixels,
        room: Option<Pixels>,
    ) -> Pixels {
        let Some(room) = room else { return bare };
        if self.spaces_are_content() || head >= end {
            return bare;
        }
        let body = self.text[head..end]
            .trim_end_matches(['\n', '\u{b}', '\u{c}', '\u{85}', '\u{2028}', '\u{2029}']);
        let body_end = head + body.len();
        // Начало хвостового ряда СОХРАНЁННЫХ пробелов переносящего куска.
        let mut from = body_end;
        for (i, ch) in body.char_indices().rev() {
            let w = self.wrap_at(head + i);
            if matches!(ch, ' ' | '\t') && w.keep_spaces && !w.nowrap && !w.break_spaces {
                from = head + i;
            } else {
                break;
            }
        }
        if from == body_end || self.span(segs, head, from) >= room {
            return bare;
        }
        let full = self.span(segs, head, body_end) - self.tail_spacing(body_end);
        let fit = if full < room { full } else { room };
        if fit > bare { fit } else { bare }
    }

    /// Место разрыва внутри неразрывного куска — по знакам, до последнего
    /// влезающего.
    pub(crate) fn cut_by_char(
        &self,
        start: usize,
        end: usize,
        limit: Option<Pixels>,
        x: &dyn Fn(usize) -> Pixels,
    ) -> usize {
        let Some(limit) = limit else { return end };
        let from = x(start);
        let mut last = start;
        // Конец отрезка — тоже граница знака, и проверять его ОБЯЗАТЕЛЬНО:
        // без него разрез, у которого не влезал только последний знак,
        // возвращал весь отрезок целиком. При `break-spaces` это съедало
        // ведущий пробел следующей строки — он уезжал в конец предыдущей.
        let bounds = self.text[start..end]
            .char_indices()
            .map(|(i, _)| start + i)
            .chain(std::iter::once(end));
        for at in bounds {
            if at == start {
                continue;
            }
            // Рвать ВНУТРИ грозди знаков нельзя: огласовка, соединитель и
            // знак вариации принадлежат своей букве и в другую строку не
            // уходят (`overflow-wrap-cluster`: देवनागरी рвалась пополам).
            if !cluster_edge(&self.text, at) {
                continue;
            }
            // И только там, где аварийный разрыв РАЗРЕШЁН: у соседнего куска
            // правила могут быть другими.
            if !self.emergency_ok(at.saturating_sub(1)) && !self.emergency_ok(at) {
                continue;
            }
            // `word-break: break-all` рвёт между БУКВАМИ и запретов типографики
            // не отменяет: перед точкой и после знака-приставки строка не
            // рвётся даже в аварийном разрезе (`word-break-break-all-inline-008`
            // — «X» и «.» обязаны остаться вместе и вылезти за коробку).
            // Семейство `anywhere` — наоборот, рвёт где угодно.
            if !self.loose_at(at.saturating_sub(1)) && !self.loose_at(at) {
                let after = self.text[at..].chars().next();
                let before = self.text[..at].chars().next_back();
                if after.is_some_and(no_break_before) || before.is_some_and(no_break_after) {
                    continue;
                }
            }
            if x(at) - from > limit {
                return if last > start { last } else { at };
            }
            last = at;
        }
        // Хвост за последней разрешённой точкой не влез — разрыв по ней.
        // Прежде возвращался весь отрезок: ряд «XX<span>XX</span>XXX» с
        // `overflow-wrap: anywhere` на `<span>` рвался внутри него, но
        // последняя точка (между `<span>` и хвостом) терялась, и хвост
        // уезжал за край вместе с частью `<span>`.
        if last > start && x(end) - from > limit {
            return last;
        }
        end
    }

    /// Есть ли между `start` и `end` кусок с разрешённым аварийным разрывом
    /// (см. `emergency_ok`).
    pub(crate) fn emergency_inside(&self, start: usize, end: usize) -> bool {
        self.spans.iter().any(|(r, w)| {
            r.start < end
                && r.end > start
                && !w.nowrap
                && (w.break_all || w.anywhere || w.break_word || w.wrap_anywhere)
        })
    }

    /// Конец измеряемой части строки: висящий хвост срезается ПО МЕСТУ.
    ///
    /// Пробел куска с `break-spaces` (и `pre`) не висит (css-text-3 §4.1.3:
    /// «treated the same as other visible characters»), а висеть может только
    /// то, что стоит у самого края, — значит, и всё ПЕРЕД ним остаётся в
    /// строке (`hanging-whitespace-001`: U+3000 абзаца `normal` перед
    /// `<span style="white-space:break-spaces"> </span>`). `spaces_are_content`
    /// смотрит правило абзаца и вложенного куска не видит. Без таких кусков
    /// результат совпадает с `trim_hanging`.
    pub(crate) fn hang_tail(&self, start: usize, end: usize) -> usize {
        // A ruby base/annotation unit is laid out by its own sub-line breaker
        // (Blink line_breaker.cc: ruby columns), whose trailing spaces do not
        // hang: `<ruby>　　あ　　<rt>…</ruby>` keeps its 5em base
        // (`ruby-overhang-spaces-*-ref`).
        if self.ruby_unit {
            return end;
        }
        let mut at = end;
        for (i, ch) in self.text[start..end].char_indices().rev() {
            if ch == '\u{feff}' || !(hangs(ch) || zero_width(ch)) {
                break;
            }
            if ch != '\n' && hangs(ch) {
                let w = self.wrap_at(start + i);
                if w.break_spaces || (w.keep_spaces && w.nowrap) {
                    break;
                }
            }
            at = start + i;
        }
        at
    }

    /// Неперносима ли точка МЕЖДУ двумя знаками.
    ///
    /// Решает её общий предок (css-text-3 §5.1). У нас предки выражены
    /// диапазонами кусков: если оба знака в ОДНОМ куске, правило его; если в
    /// разных (или один вне кусков) — общий предок это сам абзац.
    pub(crate) fn nowrap_between(&self, at: usize) -> bool {
        let which = |i: usize| self.spans.iter().position(|(r, _)| r.contains(&i));
        let left = which(at.saturating_sub(1));
        let right = which(at);
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.10): точку переноса после ПРОБЕЛА решает
        // `white-space` элемента с самим пробелом (css-text-3 §5.1, «for soft
        // wrap opportunities created by characters that disappear or are
        // preserved spaces»), а не общий предок. Срез 1973 пар (css-text,
        // CSS2/text, <pre>): +1/−3 — white-space-wrap-after-nowrap-001
        // 0.62 -> 0.28, но white-space-007 0.04 -> 11.99,
        // white-space-collapsing-breaks-001 0.00 -> «красное видно». Причина
        // не разобрана; подозрение — после схлопывания через границу куска
        // (`collapse_across_pieces`) уцелевший пробел лежит не в том куске,
        // что у браузера.
        match (left, right) {
            (Some(a), Some(b)) if a == b => self.spans[a].1.nowrap,
            _ => self.wrap.nowrap,
        }
    }

    /// Конец строки после шага 3 Phase II (css-text-3 §4.1.3): «A sequence of
    /// collapsible spaces at the end of a line … is removed». Удаляется из
    /// СТРОКИ, а не только из её ширины: подложка куска больше не тянется по
    /// пробелу (`line-break-anywhere-and-white-space-004`). Схлопываемые —
    /// U+0020, табуляция и U+1680 при normal/nowrap/pre-line (§4.1.3); U+3000,
    /// U+00A0 и прочие Zs не схлопываются, они ВИСЯТ и рисуются (★ откат у
    /// `trim_hanging`: обрезка подложки по всему `hangs` ломала
    /// `trailing-ideographic-space-*`).
    pub(crate) fn drop_collapsible_tail(&self, start: usize, end: usize) -> usize {
        let mut at = end;
        for (i, ch) in self.text[start..end].char_indices().rev() {
            if matches!(ch, ' ' | '\t' | '\u{1680}') && !self.wrap_at(start + i).keep_spaces {
                at = start + i;
            } else {
                break;
            }
        }
        at
    }

    /// Рвётся ли на этом месте что угодно и где угодно — без оглядки на
    /// типографику (`line-break: anywhere`, `overflow-wrap: anywhere`,
    /// `word-wrap: break-word`).
    pub(crate) fn loose_at(&self, at: usize) -> bool {
        let w = self.wrap_at(at);
        w.anywhere || w.break_word || w.wrap_anywhere
    }

    /// Разрешён ли на этом месте аварийный разрыв по знакам.
    pub(crate) fn emergency_ok(&self, at: usize) -> bool {
        let w = self.wrap_at(at);
        !w.nowrap && (w.break_all || w.anywhere || w.break_word || w.wrap_anywhere)
    }

    /// Точки, где строку РАЗРЕШЕНО разорвать.
    /// Правила переноса, действующие на байте `at`: сначала свой кусок, потом
    /// абзац целиком.
    /// Сохранённые пробелы конца строки — СОДЕРЖИМОЕ, а не висящие: при
    /// `break-spaces` (они занимают место и дают разрыв) и при `pre`
    /// (css-text-3 §4.1.3 висят только `normal`/`nowrap`/`pre-line` — без
    /// условий — и `pre-wrap` — условно; `pre` в списке нет:
    /// `white-space-intrinsic-size-015`, эталон `eol-spaces-bidi-004`).
    pub(crate) fn spaces_are_content(&self) -> bool {
        self.wrap.break_spaces || (self.wrap.keep_spaces && self.wrap.nowrap)
    }

    pub(crate) fn wrap_at(&self, at: usize) -> Wrap {
        self.spans
            .iter()
            .find(|(r, _)| r.contains(&at))
            .map(|(_, w)| *w)
            .unwrap_or(self.wrap)
    }

    /// Точки переноса по UAX-14 — по тексту БЕЗ знаков-распорок.
    ///
    /// Распорка (`inline::SPACER`) — не знак документа, а место под поля
    /// строчной коробки. Класс WJ запрещает разрыв и перед собой, поэтому
    /// пробел ПЕРЕД `<span>` с отступом переставал быть точкой переноса, и
    /// строка уходила за край коробки вместо переноса. Разрыв возвращается на
    /// место распорки: поле уезжает на новую строку вместе со своим текстом.
    pub(crate) fn linebreaks(&self) -> Vec<(usize, unicode_linebreak::BreakOpportunity)> {
        let mut out = self.linebreaks_uax();
        // Атом — точка переноса с обеих сторон (css-text-3 §5.1: для переноса
        // атом — знак-заместитель объекта; Blink `line_breaker.cc` рвёт до и
        // после атомарной коробки). Нельзя только рядом со знаками GL/WJ/ZWJ
        // — «with the exception of U+00A0 NO-BREAK SPACE» (§5.1
        // «atomic-compat-wrap»): рядом с ним разрыв, наоборот, есть
        // (`line-breaking-atomic-001/002`). Рядом с пробелом точку даёт сам
        // UAX #14 — после ряда пробелов, а не перед ним.
        if !self.atom_boxes.is_empty() {
            let glue = |ch: char| {
                use unicode_linebreak::BreakClass::*;
                ch != '\u{a0}'
                    && matches!(
                        unicode_linebreak::break_property(ch as u32),
                        NonBreakingGlue | WordJoiner | ZeroWidthJoiner
                    )
            };
            // Пунктуация разрыв у атома НЕ гасит: «there is a soft wrap
            // opportunity before and after each replaced element or other
            // atomic inline, even when adjacent to a character that would
            // normally suppress them» (css-text-3 §5.1;
            // `line-breaking-replaced-006`: `<img>:` рвётся перед двоеточием).
            let space = |ch: char| matches!(ch, ' ' | '\t' | '\n' | '\u{200b}');
            // Соседний знак — мимо распорок полей (их перенос не видит, см.
            // `linebreaks_uax`); соседний атом читается знаком-заместителем.
            let atom_at = |at: usize| self.atom_boxes.iter().any(|x| x.at == at);
            let skip = |at: usize| self.spacers.binary_search(&at).is_ok();
            let prev_of = |mut at: usize| -> Option<char> {
                loop {
                    let (i, ch) = self.text[..at].char_indices().next_back()?;
                    if atom_at(i) {
                        return Some('\u{fffc}');
                    }
                    if !skip(i) {
                        return Some(ch);
                    }
                    at = i;
                }
            };
            let next_of = |mut at: usize| -> Option<char> {
                loop {
                    let ch = self.text.get(at..)?.chars().next()?;
                    if atom_at(at) {
                        return Some('\u{fffc}');
                    }
                    if !skip(at) {
                        return Some(ch);
                    }
                    at += ch.len_utf8();
                }
            };
            for b in &self.atom_boxes {
                let end = b.at + 3;
                if let Some(prev) = prev_of(b.at)
                    && !space(prev)
                    && !glue(prev)
                {
                    out.push((b.at, unicode_linebreak::BreakOpportunity::Allowed));
                }
                if let Some(next) = next_of(end)
                    && !space(next)
                    && !glue(next)
                {
                    out.push((end, unicode_linebreak::BreakOpportunity::Allowed));
                }
            }
            out.sort_by_key(|(at, _)| *at);
            out.dedup_by_key(|(at, _)| *at);
        }
        out
    }

    pub(crate) fn linebreaks_uax(&self) -> Vec<(usize, unicode_linebreak::BreakOpportunity)> {
        // Распорка атома читается как U+FFFC: класс CB даёт разрыв до и после
        // (UAX #14 LB20; css-text-3 §5.1 — для переноса атом как знак-
        // заместитель объекта, Blink `inline_items_builder.cc`). Длина в
        // UTF-8 у U+FEFF и U+FFFC одна — смещения не съезжают.
        let replaced;
        let text: &str = if self.atom_boxes.is_empty() {
            &self.text
        } else {
            let mut t = self.text.to_string();
            for b in &self.atom_boxes {
                if t.get(b.at..b.at + 3) == Some("\u{feff}") {
                    t.replace_range(b.at..b.at + 3, "\u{fffc}");
                }
            }
            replaced = t;
            &replaced
        };
        if self.spacers.is_empty() {
            return unicode_linebreak::linebreaks(text).collect();
        }
        let mut clean = String::with_capacity(text.len());
        let mut map: Vec<usize> = Vec::with_capacity(text.len() + 1);
        let mut pending: Option<usize> = None;
        for (at, ch) in text.char_indices() {
            if self.spacers.binary_search(&at).is_ok() {
                pending.get_or_insert(at);
                continue;
            }
            map.push(pending.take().unwrap_or(at));
            for k in 1..ch.len_utf8() {
                map.push(at + k);
            }
            clean.push(ch);
        }
        map.push(self.text.len());
        unicode_linebreak::linebreaks(&clean)
            .map(|(at, kind)| (map.get(at).copied().unwrap_or(self.text.len()), kind))
            .collect()
    }

    pub(crate) fn opportunities(&self) -> Vec<Stop> {
        let mut out: Vec<Stop> = Vec::new();
        // Обязательные разрывы есть всегда, даже при `nowrap`.
        for (i, ch) in self.text.char_indices() {
            if ch == '\n' {
                out.push(Stop {
                    at: i + 1,
                    mandatory: true,
                });
            }
        }
        {
            if self.wrap.anywhere {
                for (i, _) in self.text.char_indices().skip(1) {
                    out.push(Stop {
                        at: i,
                        mandatory: false,
                    });
                }
            } else {
                // `line-break: anywhere` на вложенном куске: точки ставятся
                // только внутри него, остальной абзац живёт по UAX-14.
                for (range, w) in &self.spans {
                    if !w.anywhere {
                        continue;
                    }
                    for (i, _) in self.text[range.clone()].char_indices().skip(1) {
                        out.push(Stop {
                            at: range.start + i,
                            mandatory: false,
                        });
                    }
                }
                for (at, kind) in self.linebreaks() {
                    // Обязательные разрывы Юникода — это не только перевод
                    // строки: подача страницы, вертикальная табуляция,
                    // разделители строки и абзаца, NEL. Все они заканчивают
                    // строку принудительно (`line-breaking-022`).
                    match kind {
                        unicode_linebreak::BreakOpportunity::Allowed => out.push(Stop {
                            at,
                            mandatory: false,
                        }),
                        // Конец текста переносчик тоже зовёт обязательным
                        // разрывом — но переносить там нечего, а лишняя точка
                        // ломает счёт строк.
                        unicode_linebreak::BreakOpportunity::Mandatory if at < self.text.len() => {
                            out.push(Stop {
                                at,
                                mandatory: true,
                            })
                        }
                        unicode_linebreak::BreakOpportunity::Mandatory => {}
                    }
                }
                {
                    // Разрыв разрешён между знаками слова, но запреты
                    // типографики он не отменяет: перед точкой, скобкой или
                    // знаком препинания рвать всё равно нельзя (это отличает
                    // `break-all` от `line-break: anywhere`). Правило берётся
                    // ПО МЕСТУ: заданное на вложенном `<span>`, оно действует
                    // только на его байты.
                    for (i, ch) in self.text.char_indices().skip(1) {
                        if !self.wrap_at(i).break_all {
                            continue;
                        }
                        let before = self.text[..i].chars().next_back();
                        let allowed = !ch.is_whitespace()
                            && !no_break_before(ch)
                            && before.is_none_or(|c| !no_break_after(c));
                        if allowed {
                            out.push(Stop {
                                at: i,
                                mandatory: false,
                            });
                        }
                    }
                }
                {
                    // `line-break: normal`/`loose` (css-text-3 §5.2): перед
                    // малой каной и знаком долготы (класс CJ) разрыв
                    // разрешён — UAX-14 в строгом варианте держит их при
                    // предыдущем знаке. В `loose` ещё перед знаками повтора и
                    // между неразделимыми `‥…` (класс IN). Слева — не пробел
                    // и не открывающая скобка/запрет (`line-break-loose-011`,
                    // `line-break-normal-011`). Уровень берётся ПО МЕСТУ.
                    for (i, ch) in self.text.char_indices().skip(1) {
                        let level = self.wrap_at(i).loose;
                        if level == 0 {
                            continue;
                        }
                        let Some(before) = self.text[..i].chars().next_back() else {
                            continue;
                        };
                        let cjk = self.wrap_at(i).cjk_lang;
                        let eased = conditional_japanese_starter(ch)
                            || (level >= 2
                                && (iteration_mark(ch)
                                    || (matches!(ch, '\u{2025}' | '\u{2026}')
                                        && matches!(before, '\u{2025}' | '\u{2026}'))))
                            // Только для китайского/японского письма (§5.2):
                            // `normal`/`loose` — перед волнистым тире
                            // U+301C/U+30A0; `loose` — перед центрированной
                            // пунктуацией, перед широкими постфиксами (PO)
                            // и ПОСЛЕ широких префиксов (PR)
                            // (`line-break-loose-016a/016b/017a/017b/018`).
                            || (cjk && matches!(ch, '\u{301C}' | '\u{30A0}'))
                            || (cjk
                                && level >= 2
                                && (centered_punctuation(ch) || wide_postfix(ch)));
                        // После широкого префикса запрет UAX-14 «PR × ID»
                        // снимается целиком — проверяется только правый знак.
                        let after_prefix = cjk
                            && level >= 2
                            && wide_prefix(before)
                            && !ch.is_whitespace()
                            && !no_break_before(ch);
                        if after_prefix
                            || (eased
                                && !before.is_whitespace()
                                && !no_break_after(before)
                                && !matches!(before, '\u{200B}' | '\u{2060}' | '\u{00A0}'))
                        {
                            out.push(Stop {
                                at: i,
                                mandatory: false,
                            });
                        }
                    }
                }
                {
                    // Мягкий перенос — ЯВНАЯ точка переноса: она сильнее
                    // запретов типографики. UAX-14 держит вместе перенос и
                    // следующую за ним кавычку (`hyphens-i18n-manual-003`:
                    // «tú­’àn» не рвалось вовсе).
                    for (i, ch) in self.text.char_indices() {
                        if ch == SOFT_HYPHEN {
                            out.push(Stop {
                                at: i + ch.len_utf8(),
                                mandatory: false,
                            });
                        }
                    }
                }
                {
                    // После КАЖДОГО сохранённого пробела — своя точка разрыва.
                    // Табуляция тоже пробел: `break-spaces` рвёт и после неё,
                    // хотя UAX-14 держит подряд идущие табуляции вместе
                    // (`break-spaces-tab-003`).
                    // Пробел здесь — ЛЮБОЙ сохранённый пробельный знак:
                    // `break-spaces` рвёт и после идеографического U+3000,
                    // и после em-space U+2003 (`break-spaces-with-ideographic-
                    // space-005/010`, `trailing-ideographic-space-break-spaces-007`
                    // показывали красное). Перевод строки исключён: его разрыв
                    // обязательный и ставится своим проходом.
                    for (i, ch) in self.text.char_indices() {
                        if ch.is_whitespace()
                            && ch != '\n'
                            && ch != '\r'
                            && self.wrap_at(i).break_spaces
                        {
                            out.push(Stop {
                                at: i + ch.len_utf8(),
                                mandatory: false,
                            });
                        }
                    }
                }
                {
                    // Запрет действует только МЕЖДУ буквенными единицами:
                    // иероглифы друг от друга не отрываются, а после запятой
                    // или дефиса строка рвётся по-прежнему.
                    out.retain(|s| {
                        if !self.wrap_at(s.at).keep_all {
                            return true;
                        }
                        let before = self.text[..s.at].chars().next_back();
                        let after = self.text[s.at..].chars().next();
                        s.mandatory
                            || !(before.is_some_and(letter_unit) && after.is_some_and(letter_unit))
                    });
                }
            }
        }
        // `white-space: nowrap`/`pre` гасит МЯГКИЕ точки — но по месту, а не по
        // абзацу целиком: вложенный `<span>` со своим `white-space` переносится
        // внутри неперносимого абзаца и наоборот (`white-space-pre-031`).
        //
        // Точку между ДВУМЯ знаками решает их общий предок (css-text-3
        // §5.1), поэтому гасится она, только если неперносимы ОБЕ стороны.
        // Пока смотрели один знак слева, `<span style="white-space:pre">口</span>口`
        // не рвался на границе куска, хотя рвать там велит div-родитель
        // (`line-breaking-ic-001`).
        out.retain(|s| s.mandatory || !self.nowrap_between(s.at));
        // Внутри грозди знаков рвать нельзя НИКОГДА: составной знак (флаг,
        // смайлик с модификатором) — одна буква, и переносчик UAX-14 о его
        // устройстве не знает (`line-breaking-014`: радужный флаг рассыпался
        // на четыре строки).
        // Обязательный разрыв не снимается НИКОГДА: перевод строки обязан
        // закончить строку, иначе набор получает строку с переводом внутри и
        // падает на проверке (`text argument should not contain newlines`).
        // `line-break: anywhere` рвёт между ЛЮБЫМИ знаками, включая склеенные
        // соединителем нулевой ширины: класс ZWJ он тоже перекрывает
        // (`line-break-anywhere-overrides-uax-behavior-015`).
        out.retain(|s| {
            s.mandatory || cluster_edge_at(&self.text, s.at, self.wrap_at(s.at).anywhere)
        });
        // Знак перед числом держит следующий за собой: `$`, `£`, `\`. Таблица
        // пар UAX-14 в переносчике этого не знает и рвёт «XX XX\\\» между
        // обратными косыми (`word-break-break-all-023`), тогда как рвать
        // разрешено только ПЕРЕД первой из них.
        // `line-break: anywhere` и `overflow-wrap: anywhere` снимают запреты
        // типографики целиком — их точки остаются.
        // `line-break: loose` в китайском/японском письме снимает запрет и
        // после ШИРОКИХ префиксов (css-text-3 §5.2: «breaks after prefixes
        // (PR) with East Asian Width A/F/W», `line-break-loose-018`).
        out.retain(|s| {
            let w = self.wrap_at(s.at);
            // Распорка атома (U+FEFF, класс WJ) здесь не запрет: за атомом
            // точку ставит `linebreaks`.
            s.mandatory
                || w.anywhere
                || w.wrap_anywhere
                || self
                    .atom_boxes
                    .iter()
                    .any(|b| b.at + 3 == s.at || b.at == s.at)
                || self.text[..s.at].chars().next_back().is_none_or(|c| {
                    !no_break_after(c) || (w.loose >= 2 && w.cjk_lang && wide_prefix(c))
                })
        });
        out.sort_by_key(|s| (s.at, !s.mandatory));
        out.dedup_by_key(|s| s.at);
        out
    }
}

/// Открывающий знак — скобка или кавычка.
pub(super) fn is_opening(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        OpenPunctuation | Quotation
    )
}

/// Закрывающий знак — скобка или кавычка.
pub(super) fn is_closing(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        ClosePunctuation | CloseParenthesis | Quotation
    )
}

/// Точка или запятая — то, что свисает по `force-end`/`allow-end`.
pub(super) fn is_stop(ch: char) -> bool {
    matches!(
        ch,
        '.' | ','
            | '\u{060C}'
            | '\u{06D4}'
            | '、'
            | '。'
            | '，'
            | '．'
            | '\u{FE50}'
            | '\u{FE51}'
            | '\u{FE52}'
            | '\u{FF61}'
            | '\u{FF64}'
    )
}

/// Буквенная единица письма: между такими знаками `word-break: keep-all`
/// запрещает разрыв. Знаки препинания сюда не входят — после них рвать можно.
///
/// Пробел единицей письма не является НИКАКОЙ, даже идеографический: по
/// классу переноса он иероглиф (ID), и запрет заодно снимал перенос по нему
/// (`word-space-transform-013`: коробка шла одной строкой за край).
fn letter_unit(ch: char) -> bool {
    if ch.is_whitespace() {
        return false;
    }
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        Alphabetic
            | Numeric
            | Ambiguous
            | Ideographic
            | ConditionalJapaneseStarter
            | HebrewLetter
            | ComplexContext
            | CombiningMark
            | HangulLvSyllable
            | HangulLvtSyllable
            | HangulLJamo
            | HangulVJamo
            | HangulTJamo
    )
}

/// Знак, перед которым рвать нельзя: закрывающая скобка, знак препинания,
/// разделитель разрядов, неразрывный пробел (классы UAX-14).
fn no_break_before(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        ClosePunctuation
            | CloseParenthesis
            | Exclamation
            | InfixSeparator
            | NonStarter
            | Symbol
            | NonBreakingGlue
            | WordJoiner
            | ZeroWidthJoiner
    )
}

/// Знак, после которого рвать нельзя: открывающая скобка, склейка, знак
/// перед числом (`$`, `\`, `£`).
///
/// Соединитель нулевой ширины держит составные знаки вместе — на нём собраны
/// целые эмодзи (человек + компьютер = «программист»). Разрыв по нему
/// рассыпал бы один знак на составные части.
fn no_break_after(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        OpenPunctuation | NonBreakingGlue | WordJoiner | ZeroWidthJoiner | Prefix
    )
}

/// Правила переноса из стиля БЕЗ вопроса, нужна ли своя раскладка.
pub fn wrap_of(c: &crate::style::computed::Computed) -> Wrap {
    Wrap {
        nowrap: c.nowrap == Some(true),
        break_spaces: c.break_after_spaces == Some(true),
        break_all: c.break_anywhere == Some(true) && c.break_anywhere_strict != Some(true),
        anywhere: c.break_anywhere_strict == Some(true),
        keep_all: c.keep_all == Some(true),
        break_word: c.break_word == Some(true),
        wrap_anywhere: c.wrap_anywhere == Some(true),
        rtl: c.rtl == Some(true),
        balance: c.balance_lines == Some(true),
        keep_spaces: c.keep_spaces == Some(true),
        loose: c.line_break_loose.unwrap_or(0),
        cjk_lang: c
            .lang
            .as_deref()
            .is_some_and(|l| l.starts_with("ja") || l.starts_with("zh")),
    }
}

/// Центрированная пунктуация (css-text-3 §5.2, `loose` в ja/zh): U+30FB,
/// U+FF1A, U+FF1B, U+FF65, U+203C, U+2047-2049, U+FF01, U+FF1F.
fn centered_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '\u{30FB}' | '\u{FF1A}' | '\u{FF1B}' | '\u{FF65}' | '\u{203C}' | '\u{2047}'
            ..='\u{2049}' | '\u{FF01}' | '\u{FF1F}'
    )
}

/// Постфиксы класса PO с восточноазиатской шириной A/F/W (§5.2, `loose` в
/// ja/zh): °, ‰, ℃, ％ и полноширинные знаки процента/цента.
fn wide_postfix(ch: char) -> bool {
    matches!(
        ch,
        '\u{00B0}'
            | '\u{2030}'
            | '\u{2031}'
            | '\u{2103}'
            | '\u{2109}'
            | '\u{FF05}'
            | '\u{FFE0}'
            | '\u{2032}'
            | '\u{2033}'
    )
}

/// Префиксы класса PR с шириной A/F/W (§5.2, `loose` в ja/zh): €, №, ￥, ￡,
/// ＄, ₩, §, ¶.
fn wide_prefix(ch: char) -> bool {
    matches!(
        ch,
        '\u{20AC}'
            | '\u{2116}'
            | '\u{FFE5}'
            | '\u{FFE1}'
            | '\u{FF04}'
            | '\u{FFE6}'
            | '\u{00A7}'
            | '\u{00B6}'
            | '\u{20A9}'
    )
}

/// Класс CJ по UAX-14: малая кана, знак долготы, их полуширинные формы.
/// `unicode_linebreak` разрешает CJ как NS (строгий вариант) — перед ними
/// разрыва нет; `line-break: normal`/`loose` его возвращают (css-text-3 §5.2:
/// «breaks before Japanese small kana or the Katakana-Hiragana prolonged
/// sound mark, i.e. characters from the Unicode line breaking class CJ»).
fn conditional_japanese_starter(ch: char) -> bool {
    matches!(
        ch,
        '\u{3041}' | '\u{3043}' | '\u{3045}' | '\u{3047}' | '\u{3049}' | '\u{3063}'
            | '\u{3083}' | '\u{3085}' | '\u{3087}' | '\u{308E}' | '\u{3095}' | '\u{3096}'
            | '\u{30A1}' | '\u{30A3}' | '\u{30A5}' | '\u{30A7}' | '\u{30A9}' | '\u{30C3}'
            | '\u{30E3}' | '\u{30E5}' | '\u{30E7}' | '\u{30EE}' | '\u{30F5}' | '\u{30F6}'
            | '\u{30FC}'
            | '\u{31F0}'..='\u{31FF}'
            | '\u{FF67}'..='\u{FF70}'
    )
}

/// Знаки повтора (css-text-3 §5.2, только `loose`): U+3005, U+303B, U+309D,
/// U+309E, U+30FD, U+30FE.
fn iteration_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{3005}' | '\u{303B}' | '\u{309D}' | '\u{309E}' | '\u{30FD}' | '\u{30FE}'
    )
}

/// Можно ли разорвать текст ровно на этом месте — граница ли это грозди.
///
/// Смотрятся ОБЕ стороны: знак справа не должен быть продолжением
/// (огласовка, модификатор, знак-тег), а знак слева не должен быть
/// соединителем — после нулевого соединителя гроздь продолжается следующим
/// знаком (`line-breaking-014`: радужный флаг рвался по соединителю).
pub(super) fn cluster_edge(text: &str, at: usize) -> bool {
    if at >= text.len() {
        return true;
    }
    // Огласовка ПОСЛЕ пробела ни к чему не приросла: по UAX-14 (правило LB9)
    // знак-продолжение после разделителя считается обычной буквой, и рвать
    // перед ним можно.
    let before = text[..at].chars().next_back();
    if before.is_none_or(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
        return true;
    }
    if !cluster_start(&text[at..]) {
        return false;
    }
    !matches!(text[..at].chars().next_back(), Some('\u{200d}'))
}

/// То же, но с учётом `line-break: anywhere`.
///
/// `anywhere` перекрывает класс ZWJ по css-text-4, то есть рвать РЯДОМ с
/// соединителем можно. Саму гроздь он не разбирает: огласовка, знак вариации,
/// модификатор тона и знак-тег остаются при своём знаке, иначе эмодзи-цепочка
/// рассыпается по строкам (`line-breaking-014`).
fn cluster_edge_at(text: &str, at: usize, anywhere: bool) -> bool {
    if !anywhere {
        return cluster_edge(text, at);
    }
    if at >= text.len() {
        return true;
    }
    let before = text[..at].chars().next_back();
    if before.is_none_or(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
        return true;
    }
    let next = text[at..].chars().next();
    next == Some('\u{200d}') || before == Some('\u{200d}') || cluster_start(&text[at..])
}

/// Начинается ли с этого места ГРОЗДЬ знаков — то есть можно ли тут рвать.
///
/// Знаки-продолжения грозди: соединительная огласовка (класс CM по UAX-14),
/// нулевой соединитель и знаки вариации.
fn cluster_start(rest: &str) -> bool {
    let Some(ch) = rest.chars().next() else {
        return true;
    };
    // Продолжения грозди: нулевой соединитель, знаки вариации, знаки-теги
    // (флаги вроде уэльского), модификаторы тона кожи. Все они принадлежат
    // предыдущему знаку и в другую строку не уходят (`line-breaking-014`).
    if matches!(
        ch as u32,
        0x200D
            | 0xFE00..=0xFE0F
            | 0xE0100..=0xE01EF
            | 0xE0020..=0xE007F
            | 0x1F3FB..=0x1F3FF
    ) {
        return false;
    }
    !matches!(
        unicode_linebreak::break_property(ch as u32),
        unicode_linebreak::BreakClass::CombiningMark
    )
}
