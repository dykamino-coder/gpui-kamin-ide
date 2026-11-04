//! Межбуквенный интервал последнего знака строки по css-text-3 §8.2.

use crate::text::paragraph::Paragraph;
use gpui::{Pixels, px};

impl Paragraph {
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
        if self.text[..end].ends_with('\u{feff}') {
            return px(0.);
        }
        // Принудительный разрыв единицей письма не является: трекинг конца
        // строки — у последнего знака ПЕРЕД ним (css-text-3 §8.2). Пока брался
        // трекинг самого `\n`, строка с `<span>` вокруг перевода строки
        // теряла или сохраняла лишний зазор (`letter-spacing-end-of-line-002`).
        let body = self.text[..end].trim_end_matches('\n');
        if body.is_empty() {
            return px(0.);
        }
        let (at, last) = body.char_indices().next_back().unwrap_or((0, ' '));
        // Before the `\n` may sit an inline box margin spacer or an atom
        // carrier: their "tracking" is the box advance, not letter-spacing,
        // and must not be removed (`text-indent-intrinsic-*`,
        // `word-space-transform-011`).
        if body.len() < end
            && (matches!(last, '\u{feff}' | '\u{fffc}' | '\u{200b}')
                || self.atom_boxes.iter().any(|b| b.at == at)
                || self.spacers.contains(&at))
        {
            return px(0.);
        }
        self.prepared()
            .letters
            .first(at)
            .map_or(self.letter_spacing, |i| self.letter_spans[i].1)
    }
}
