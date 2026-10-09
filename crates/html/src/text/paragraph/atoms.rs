//! Атомы в строке: места, базовые линии, высоты строк.

use crate::text::paragraph::*;
use gpui::{AnyElement, App, IntoElement, Pixels, Window, px, size};

impl Paragraph {
    /// Атомы в строке: место в тексте (байт распорки) → элемент и его
    /// `vertical-align`. Каждый заворачивается в ряд `align-items: baseline`
    /// со щупом базовой линии (см. `BaselineProbe`): обёртка обтягивает
    /// коробку полей атома, щуп отдаёт её базовую линию.
    pub fn atoms(mut self, atoms: Vec<(usize, AnyElement, AtomAlign, RubyExtents)>) -> Self {
        use gpui::{ParentElement, Styled};
        // Распорка атома — не распорка полей: из текста для переноса её не
        // вынимают, а читают знаком-заместителем (см. `linebreaks`).
        self.spacers
            .retain(|s| !atoms.iter().any(|(at, _, _, _)| at == s));
        self.atoms = atoms
            .into_iter()
            .map(|(at, el, align, extents)| {
                let probe = std::rc::Rc::new(std::cell::Cell::new(None));
                let root = std::rc::Rc::new(std::cell::Cell::new(None));
                let el = LayoutTap {
                    child: gpui::div()
                        .flex()
                        .items_baseline()
                        .child(BaselineProbe {
                            slot: probe.clone(),
                        })
                        .child(el)
                        .into_any_element(),
                    slot: root.clone(),
                }
                .into_any_element();
                AtomSlot {
                    at,
                    el,
                    align,
                    probe,
                    root,
                    extents,
                    hidden: false,
                }
            })
            .collect();
        self
    }

    /// Верх атома от базовой линии строки (ось вниз) — для всех выравниваний,
    /// кроме `top`/`bottom`: те зависят от готовой строки (§10.8.1 «aligned
    /// subtree» решается после остальных).
    pub(crate) fn atom_top(&self, b: &AtomBox) -> f32 {
        let (asc, desc, xh) = self.strut;
        match b.align {
            AtomAlign::Shift(v) => -b.base - v,
            AtomAlign::Middle => -xh / 2.0 - b.h / 2.0,
            AtomAlign::TextTop => -asc,
            AtomAlign::TextBottom => desc - b.h,
            AtomAlign::Top | AtomAlign::Bottom => -b.base,
        }
    }

    /// Протяжённость строки над и под её базовой линией по строчным коробкам
    /// (CSS 2.1 §10.8.1): у каждой коробки `A = (L − (a + d)) / 2 + a` над
    /// базовой и `L − A` под ней, где `L` — её `line-height`, `a`/`d` —
    /// подъём и спуск её шрифта; струт блока входит всегда. Сдвиг
    /// `vertical-align` (`shift_spans`, ось вниз) двигает коробку целиком.
    /// Blink: `inline_box_state.cc` `ComputeTextMetrics` + `line_box_fragment_
    /// builder` — та же сумма наибольших подъёма и спуска.
    pub(crate) fn line_extents(&self, range: &std::ops::Range<usize>) -> (f32, f32) {
        let lh = f32::from(self.line_height);
        let (sa, sd) = self.strut_box;
        let a_s = (lh - (sa + sd)) / 2.0 + sa;
        let (mut top, mut bot) = (a_s, lh - a_s);
        let mut at = 0usize;
        for (run, &(ra, rd)) in self.runs.iter().zip(&self.run_metrics) {
            let (s, e) = (at, at + run.len);
            at = e;
            if run.len == 0 || e <= range.start || s >= range.end {
                continue;
            }
            // Кусок у края строки равняется по готовой строке (`edge_spans`).
            if self
                .edge_spans
                .iter()
                .any(|(r, _, _)| r.start <= s && e <= r.end)
            {
                continue;
            }
            let own = self
                .box_spans
                .iter()
                .find(|(r, _)| r.contains(&s))
                .map_or(lh, |(_, v)| *v);
            let a_r = (own - (ra + rd)) / 2.0 + ra;
            let dy = self
                .shift_spans
                .iter()
                .find(|(r, _)| r.contains(&s))
                .map_or(0.0, |(_, v)| f32::from(*v));
            top = top.max(a_r - dy);
            bot = bot.max(own - a_r + dy);
        }
        (top, bot)
    }

    /// От верха струта до его базовой линии: полулидинг плюс подъём (§10.8.1).
    pub(crate) fn strut_base(&self) -> f32 {
        let (asc, desc, _) = self.strut;
        (f32::from(self.line_height) - (asc + desc)) / 2.0 + asc
    }

    /// Подъём и спуск основного шрифта каждого прогона.
    pub(crate) fn measure_runs(&self, window: &mut Window) -> Vec<(f32, f32)> {
        self.runs
            .iter()
            .map(|run| {
                let id = window.text_system().resolve_font(&run.font);
                let size = run.font_size.unwrap_or(self.font_size);
                let ts = window.text_system();
                (
                    f32::from(ts.ascent(id, size)),
                    f32::from(ts.descent(id, size)).abs(),
                )
            })
            .collect()
    }

    /// Базовая линия отрезка от верха его строки: наибольшие подъём и спуск
    /// прогонов отрезка, полулидинг от высоты строки — ровно как кладёт глифы
    /// сплошной набор (`padding_top + ascent` в `vendor/gpui/.../line.rs`).
    pub(crate) fn base_of(&self, range: &std::ops::Range<usize>) -> Option<f32> {
        if self.run_metrics.len() != self.runs.len() {
            return None;
        }
        let mut at = 0usize;
        let mut best: Option<(f32, f32)> = None;
        for (run, &(a, d)) in self.runs.iter().zip(&self.run_metrics) {
            let (s, e) = (at, at + run.len);
            at = e;
            if run.len == 0 || e <= range.start || s >= range.end {
                continue;
            }
            // Прогон у края строки в базовую линию не входит: его коробка
            // равняется по краю, а не по базовой (§10.8.1). Отрезок самого
            // такого куска своей базовой не меряет — там прогон считается.
            let edge = self.edge_spans.iter().any(|(r, _, _)| {
                r.start <= s && e <= r.end && !(r.start <= range.start && range.end <= r.end)
            });
            if edge {
                continue;
            }
            best = Some(best.map_or((a, d), |(ba, bd)| (ba.max(a), bd.max(d))));
        }
        best.map(|(a, d)| (f32::from(self.line_height) - (a + d)) / 2.0 + a)
    }

    /// Базовая линия строки: по её прогонам, без них — по струту.
    pub(crate) fn line_base(&self, range: &std::ops::Range<usize>) -> f32 {
        self.base_of(range).unwrap_or_else(|| self.strut_base())
    }

    /// Раскладка атомов ДО замера абзаца: перенос и высота строк зависят от
    /// их размеров, а внутри замера раскладывать нельзя (движок раскладки
    /// занят). Атом меряется по содержимому — в строку допускаются только
    /// атомы, чей размер от ширины строки не зависит (решает `render.rs`).
    pub(crate) fn lay_atoms(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(run) = self.runs.first() {
            let id = window.text_system().resolve_font(&run.font);
            let size = run.font_size.unwrap_or(self.font_size);
            let ts = window.text_system();
            self.strut = (
                f32::from(ts.ascent(id, size)),
                f32::from(ts.descent(id, size)).abs(),
                f32::from(ts.x_height(id, size)),
            );
        }
        self.run_metrics = self.measure_runs(window);
        self.atom_boxes.clear();
        // Ruby atoms whose annotation may overhang neighbours: resolved after
        // the loop (the computation needs the whole paragraph).
        let mut overhangs: Vec<(usize, usize, usize, f32, f32, f32, RubyOverhangInfo)> = Vec::new();
        for slot in self.atoms.iter_mut() {
            let rounded = slot.el.layout_as_root(
                size(
                    gpui::AvailableSpace::MaxContent,
                    gpui::AvailableSpace::MaxContent,
                ),
                window,
                cx,
            );
            // Размер и базовая линия — ТОЧНЫЕ, без округления к точке
            // устройства (см. `LayoutTap`); щуп стоит прямо в обёртке, и его
            // смещение от неё и есть базовая линия.
            let s = slot
                .root
                .get()
                .map(|id| window.layout_exact(id).1)
                .unwrap_or(rounded);
            let base = slot
                .probe
                .get()
                .map(|id| f32::from(window.layout_exact(id).0.y))
                .unwrap_or(f32::from(s.height));
            // Базовая на самом ВЕРХУ коробки — признак того, что раскладка
            // базовой линии не нашла (таблица с пустой ячейкой отдаёт ноль).
            // По CSS 2.1 §17.5.3 и §10.8.1 тогда это низ коробки: «If there is
            // no such line box or table-row, the baseline is the bottom of
            // content edge of the cell box» (`min-height-applies-to-014`).
            let base = if base <= 0.0 && s.height > px(0.) {
                f32::from(s.height)
            } else {
                base
            };
            // Vertical mixed/upright lines use the central dominant baseline
            // (css-inline-3 §dominant-baseline auto; Blink
            // `ComputedStyle::GetFontBaseline`): the atom's central baseline —
            // its own content's central baseline, or the middle of its margin
            // box when it has none — sits on the line's central baseline,
            // which is (ascent − descent) / 2 above the strut's alphabetic
            // baseline used by the line model here.
            let base = if self.rotated_central {
                let (asc, desc, _) = self.strut;
                let h = f32::from(s.height);
                let central = if (base - h).abs() < 0.01 { h / 2.0 } else { base };
                central + (asc - desc) / 2.0
            } else {
                base
            };
            let w = f32::from(s.width);
            // Уровни одной стороны стоят стопкой в каждой колонке; выход за
            // коробку — по самой высокой стопке.
            let (mut over, mut under) = (0.0f32, 0.0f32);
            for (below, inset, id) in &slot.extents.levels {
                let Some(id) = id.get() else { continue };
                let h = f32::from(window.layout_exact(id).1.height) - inset;
                if *below {
                    under = under.max(h);
                } else {
                    over = over.max(h);
                }
            }
            let len = self.text[slot.at..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
            if let Some(info) = &slot.extents.overhang {
                let ann_w = slot
                    .extents
                    .levels
                    .iter()
                    .filter_map(|(_, _, id)| id.get())
                    .map(|id| f32::from(window.layout_exact(id).1.width))
                    .fold(0.0f32, f32::max);
                let base_w = info.base_width.get().unwrap_or(w);
                overhangs.push((self.atom_boxes.len(), slot.at, len, w, base_w, ann_w, info.clone()));
            }
            self.atom_boxes.push(AtomBox {
                at: slot.at,
                h: f32::from(s.height),
                base,
                align: slot.align,
                over,
                under,
                shift: 0.0,
            });
            // Продвижение распорки — ширина атома. Идёт ПЕРВЫМ: поиск
            // диапазона берёт первое попадание.
            self.letter_spans.insert(0, (slot.at..slot.at + len, px(w)));
        }
        // Свес аннотации руби над соседями (css-ruby-1 §4.4): распорка
        // уже на свес, атом сдвинут влево на начальный свес.
        for (k, at, len, w, base_w, ann_w, info) in overhangs {
            let (start_oh, end_oh) = self.ruby_overhang(&info, at, len, w, base_w, ann_w, window);
            if start_oh + end_oh <= 0.0 {
                continue;
            }
            self.atom_boxes[k].shift = start_oh;
            if let Some(span) = self.letter_spans.iter_mut().find(|(r, _)| *r == (at..at + len)) {
                span.1 = px((w - start_oh - end_oh).max(0.0));
            }
        }
        self.prepare_atom_fit(window, cx);
    }

    /// Поставить атомы на места их строк: x — от продвижения до распорки
    /// плюс прижим строки (тот же, что у отрисовки), y — от базовой линии
    /// строки по `vertical-align`.
    /// Трекинг, который добавлен ПОСЛЕДНЕМУ знаку отрезка.
    ///
    /// По css-text-3 §8.2 межбуквенный интервал в конце строки не действует:
    /// он свисает за край и в ширину строки не входит. Пока входил, коробка
    /// шириной ровно в текст рвала последнее слово (`letter-spacing-200`).
    pub(crate) fn tail_spacing(&self, end: usize) -> Pixels {
        if end == 0 {
            return px(0.);
        }
        // Знак-распорка несёт ПОЛЕ строчной коробки, а не трекинг: вычитать
        // его на конце строки нельзя — иначе коробка теряет своё правое поле
        // и выходит уже на целый em (`word-space-transform-010`).
        if self.text[..end].chars().next_back() == Some('\u{feff}') {
            return px(0.);
        }
        let at = self.text[..end]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.letter_spans
            .iter()
            .find(|(r, _)| r.contains(&at))
            .map(|(_, v)| *v)
            .unwrap_or(self.letter_spacing)
    }
}
