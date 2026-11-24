//! KaminIDE patch: ширины неразрывных кусков текста для `min_content_width`.
//!
//! Минимальная ширина текста — самый широкий кусок между точками разрыва, и
//! каждый кусок меряется набором платформы (`layout_line`): так же, как его
//! потом кладёт перенос. Набор куска зависит только от шрифта, кегля и самих
//! знаков (один прогон, лигатуры рвутся), поэтому повтор того же слова под тем
//! же шрифтом даёт ту же ширину. Без памяти каждый замер min-content заново
//! набирал каждое слово (DirectWrite: свой текстовый макет на слово), и на
//! длинной статье это было десятой частью всей холодной загрузки.
//! Ключ — знаки куска; шрифт и кегль — у владельца (`LineWrapper` пула на
//! шрифт и кегль), поколение шрифтов сбрасывает память: добавленный шрифт
//! может сменить подстановку глифов.
//! Память ограничена двумя поколениями по `GENERATION_WORDS` записей (как
//! `LineLayoutCache`: текущее и прошлое), частые слова переходят в текущее.

use crate::Pixels;
use collections::HashMap;

/// Записей в одном поколении. Частые слова текста (служебные, повторы терминов)
/// укладываются в него; редкие уходят со сменой поколения.
const GENERATION_WORDS: usize = 1024;

#[derive(Default)]
pub(crate) struct WordWidths {
    font_generation: usize,
    current: HashMap<Box<str>, Pixels>,
    previous: HashMap<Box<str>, Pixels>,
}

impl WordWidths {
    /// Ширина куска `word` из памяти или из `measure` (набор платформы).
    pub(crate) fn width(
        &mut self,
        font_generation: usize,
        word: &str,
        measure: impl FnOnce() -> Pixels,
    ) -> Pixels {
        if self.font_generation != font_generation {
            self.current.clear();
            self.previous.clear();
            self.font_generation = font_generation;
        }
        if let Some(width) = self.current.get(word) {
            return *width;
        }
        let (key, width) = match self.previous.remove_entry(word) {
            Some(entry) => entry,
            None => (Box::from(word), measure()),
        };
        if self.current.len() >= GENERATION_WORDS {
            self.previous = std::mem::take(&mut self.current);
        }
        self.current.insert(key, width);
        width
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.current.len() + self.previous.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::px;

    #[test]
    fn a_repeated_word_is_measured_once_per_font_generation() {
        let mut widths = WordWidths::default();
        let mut calls = 0;
        for _ in 0..3 {
            let w = widths.width(0, "word", || {
                calls += 1;
                px(12.5)
            });
            assert_eq!(w, px(12.5));
        }
        assert_eq!(calls, 1);
        // Новый шрифт в системе: прежние ширины не годятся.
        let w = widths.width(1, "word", || {
            calls += 1;
            px(13.)
        });
        assert_eq!((w, calls), (px(13.), 2));
    }

    #[test]
    fn memory_is_bounded_by_two_generations_and_keeps_frequent_words() {
        let mut widths = WordWidths::default();
        let mut measured = 0;
        for i in 0..(GENERATION_WORDS * 5) {
            widths.width(0, &format!("w{i}"), || {
                measured += 1;
                px(1.)
            });
            // Частое слово между редкими не набирается заново.
            widths.width(0, "the", || {
                measured += 1;
                px(2.)
            });
            assert!(widths.len() <= 2 * GENERATION_WORDS);
        }
        assert_eq!(measured, GENERATION_WORDS * 5 + 1);
    }
}
