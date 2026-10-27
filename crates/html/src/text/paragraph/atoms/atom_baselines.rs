//! Базовые линии атомов и строк (CSS 2.1 §10.8.1): регистрация атомов абзаца
//! в обёртке со щупом базовой линии (`BaselineProbe`), верх атома от базовой
//! по `vertical-align`, протяжённость строки над и под базовой по строчным
//! коробкам, базовая струта, подъём/спуск прогонов и базовая линия отрезка.

use crate::hooks::IntoHooked;
use crate::text::paragraph::*;
use gpui::{AnyElement, IntoElement, Window};

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
                        .child(
                            BaselineProbe {
                                slot: probe.clone(),
                            }
                            .into_hooked(),
                        )
                        .child(el)
                        .into_any_element(),
                    slot: root.clone(),
                }
                .hooked();
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
}

impl Paragraph {
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
}

impl Paragraph {
    /// Протяжённость строки над и под её базовой линией по строчным коробкам
    /// (CSS 2.1 §10.8.1): у каждой коробки `A = (L − (a + d)) / 2 + a` над
    /// базовой и `L − A` под ней, где `L` — её `line-height`, `a`/`d` —
    /// подъём и спуск её шрифта; струт блока входит всегда. Сдвиг
    /// `vertical-align` (`shift_spans`, ось вниз) двигает коробку целиком.
    /// Blink: `inline_box_state.cc` `ComputeTextMetrics` + `line_box_fragment_
    /// builder` — та же сумма наибольших подъёма и спуска.
    pub(crate) fn line_extents(&self, range: &std::ops::Range<usize>) -> (f32, f32) {
        let (lh, (sa, sd)) = self.root_line_box(range.start);
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
}

impl Paragraph {
    /// От верха струта до его базовой линии: полулидинг плюс подъём (§10.8.1).
    pub(crate) fn strut_base(&self) -> f32 {
        let (asc, desc, _) = self.strut;
        (f32::from(self.line_height) - (asc + desc)) / 2.0 + asc
    }
}

impl Paragraph {
    /// Подъём и спуск основного шрифта каждого прогона.
    pub(crate) fn measure_runs(&self, window: &mut Window) -> Vec<(f32, f32)> {
        self.runs
            .iter()
            .map(|run| {
                let id = window.text_system().resolve_font(&run.font);
                let size = run.font_size.unwrap_or(self.font_size);
                let ts = window.text_system();
                crate::text::fonts::metric_overrides::extents(
                    &run.font,
                    f32::from(size),
                    (
                        f32::from(ts.ascent(id, size)),
                        f32::from(ts.descent(id, size)).abs(),
                    ),
                )
            })
            .collect()
    }
}

impl Paragraph {
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
}

impl Paragraph {
    /// Базовая линия строки: по её прогонам, без них — по струту.
    pub(crate) fn line_base(&self, range: &std::ops::Range<usize>) -> f32 {
        self.shared_base(range).unwrap_or_else(|| self.strut_base())
    }

    /// The alphabetic baseline the runs of a line are aligned to, from the line
    /// top. Whole-line shaping puts it by the largest run ascent and descent
    /// (`base_of`). A turned line with a central dominant baseline aligns each
    /// run's central baseline with the root inline box's instead: `shift_spans`
    /// carry every run's offset from the strut (css-writing-modes-4 §4.3,
    /// css-inline-3 §dominant-baseline). The shared baseline is then the root
    /// inline box's own, half-leading included (CSS 2 §10.8.1), and a taller
    /// run must not move it.
    pub(crate) fn shared_base(&self, range: &std::ops::Range<usize>) -> Option<f32> {
        let (lh, (sa, sd)) = self.root_line_box(range.start);
        if self.rotated_central && sa + sd > 0.0 {
            return Some((lh - (sa + sd)) / 2.0 + sa);
        }
        self.base_of(range)
    }
}
