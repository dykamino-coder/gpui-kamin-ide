//! Растяжение строк: места выключки, text-fit, масштаб.

use crate::text::paragraph::*;
use gpui::{Pixels, Window, px};

impl Paragraph {
    /// Justification opportunity BETWEEN two characters (not at a word
    /// separator, which counts on its own): css-text-3 §7.3 — `auto` expands
    /// between CJK ideographs as well (Blink `ShapeResultSpacing`, text-justify
    /// auto), `inter-character`/`distribute` between every typographic
    /// character unit except inside cursive scripts.
    ///
    /// Invisible format controls (bidi isolates, ZWSP, …) are looked
    /// through, never past `line_start`: `東&#x2066;京` keeps the opportunity
    /// before `京` (`text-align-justify-bidi-control`).
    pub(crate) fn justify_boundary(&self, line_start: usize, at: usize) -> bool {
        if self.justify_chars == 0 || self.ruby_justify || self.ruby_unit {
            return false;
        }
        if at <= line_start || at >= self.text.len() || !self.text.is_char_boundary(at) {
            return false;
        }
        let Some(b) = self.text[at..].chars().next() else {
            return false;
        };
        let Some(a) = self.text[line_start..at].chars().rev().find(|c| !invisible(*c)) else {
            return false;
        };
        let plain = |c: char| {
            !word_separator(c) && !invisible(c) && !matches!(c, '\t' | '\n' | '\u{3000}')
        };
        if !plain(a) || !plain(b) || !cluster_edge(&self.text, at) {
            return false;
        }
        if self.justify_chars == 1 {
            justify_ideograph(a) || justify_ideograph(b)
        } else {
            !(cursive_script(a) && cursive_script(b))
        }
    }

    /// Justification opportunities of the line `range` before `pos`: word
    /// separators left of it plus character boundaries up to and including
    /// `pos` (a piece starting at a boundary takes its expansion).
    pub(crate) fn justify_opps(&self, range: &std::ops::Range<usize>, pos: usize) -> usize {
        let pos = pos.clamp(range.start, range.end);
        let mut n = 0;
        for (i, ch) in self.text[range.start..pos].char_indices() {
            let at = range.start + i;
            if word_separator(ch) && !self.ruby_justify {
                n += 1;
            }
            if self.justify_boundary(range.start, at) {
                n += 1;
            }
        }
        if pos < range.end && self.justify_boundary(range.start, pos) {
            n += 1;
        }
        n
    }

    /// Подобрать кегль так, чтобы строки заполнили коробку (css-text-5).
    ///
    /// Считается по САМОЙ ШИРОКОЙ строке: увеличивать до тех пор, пока она не
    /// упрётся в край. Множитель идёт на всё, что задаёт размер набора, —
    /// кегль, интерлиньяж и разрядки, иначе строка растёт непропорционально.
    pub(crate) fn apply_fit(&mut self, limit: Pixels, window: &mut Window) {
        let Some(f) = self.fit.filter(|f| f.grow || f.shrink) else {
            return;
        };
        let lines = self.split(Some(limit), window);
        let Some(widest_line) = lines.iter().max_by(|a, b| {
            a.width
                .partial_cmp(&b.width)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) else {
            return;
        };
        let widest = widest_line.width;
        if widest <= px(0.) || limit <= px(0.) {
            return;
        }
        // Немасштабируемые части строки (css-text-5 §text-fit: интервалы в
        // точках и `em`) в подборе не участвуют: множитель считается как
        // (A + B) / A, где A — масштабируемая ширина, B — остаток места.
        // Иначе `letter-spacing: 10px` рос вместе с глифами, сумма сходилась,
        // а глифы выходили не те (`text-fit/spacing`).
        let fixed = if self.fit_spacing_scalable {
            0.0
        } else {
            let text = &self.text[widest_line.range.clone()];
            let chars = text.chars().count().max(1) as f32;
            let spaces = text.chars().filter(|c| c.is_whitespace()).count() as f32;
            f32::from(self.letter_spacing) * (chars - 1.0) + f32::from(self.word_spacing) * spaces
        };
        let scalable = f32::from(widest) - fixed;
        if scalable <= 0.0 {
            return;
        }
        let mut k = (f32::from(limit) - fixed) / scalable;
        // Процент — ЗАЖИМ множителя (css-text-5): при `grow` и ≥ 100% —
        // максимум, при `shrink` и ≤ 100% — минимум; иначе предела нет.
        // Раньше он множился как доля заполнения, и `shrink 75%` давал кегль
        // вдвое меньше нужного (`shrink-per-line-all`).
        if let Some(t) = f.target {
            if f.grow && t >= 1.0 {
                k = k.min(t);
            }
            if f.shrink && t <= 1.0 {
                k = k.max(t);
            }
        }
        if !k.is_finite() || (k > 1.0 && !f.grow) || (k < 1.0 && !f.shrink) {
            return;
        }
        if (k - 1.0).abs() < 0.001 {
            return;
        }
        remember_fit(self.measure_key(), k);
        self.scale_by(k);
    }

    /// Множитель, найденный ЗАМЕРОМ для этого же абзаца.
    ///
    /// Подбирать кегль имеет смысл только под заданный размер строки, а
    /// известен он лишь замеру: отрисовке коробка достаётся уже посчитанной, и
    /// по ней подбор пошёл бы по кругу (`text-fit/writing-mode`: кегль
    /// вырастал в размер окна).
    pub(crate) fn apply_measured_fit(&mut self) {
        if self.fit.is_none() {
            return;
        }
        let key = self.measure_key();
        if let Some(k) = FITTED.with(|c| {
            c.borrow()
                .iter()
                .find(|(hit, _)| *hit == key)
                .map(|(_, k)| *k)
        }) {
            self.scale_by(k);
        }
        self.fit = None;
    }

    /// Помножить всё, что задаёт размер набора.
    pub(crate) fn scale_by(&mut self, k: f32) {
        self.font_size = px(f32::from(self.font_size) * k);
        // Заданная длиной `line-height` от подбора не зависит (css-text-5:
        // «line-height: 1.5em … are not affected by this scaling»); растёт
        // только `normal` и число — они считаются от использованного кегля.
        if !self.fit_line_height_fixed {
            self.line_height = px(f32::from(self.line_height) * k);
            for (_, lh) in &mut self.lh_spans {
                *lh = px(f32::from(*lh) * k);
            }
        }
        if self.fit_spacing_scalable {
            self.letter_spacing = px(f32::from(self.letter_spacing) * k);
            self.word_spacing = px(f32::from(self.word_spacing) * k);
        }
        for run in &mut self.runs {
            if let Some(size) = run.font_size {
                run.font_size = Some(px(f32::from(size) * k));
            }
        }
    }
}
