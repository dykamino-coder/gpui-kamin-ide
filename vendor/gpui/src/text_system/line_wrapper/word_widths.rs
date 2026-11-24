//! KaminIDE patch: ширины неразрывных кусков текста для `min_content_width`.
//!
//! Минимальная ширина текста — самый широкий кусок между точками разрыва, и
//! каждый кусок меряется набором платформы (`layout_line`): так же, как его
//! потом кладёт перенос. Набор куска зависит только от шрифта, кегля и самих
//! знаков (один прогон, лигатуры рвутся), поэтому повтор того же слова под тем
//! же шрифтом даёт ту же ширину. Без памяти каждый замер min-content заново
//! набирал каждое слово (DirectWrite: свой текстовый макет на слово), и на
//! длинной статье это было десятой частью всей холодной загрузки.
//!
//! Ключ — знаки куска; шрифт и кегль — у владельца (`LineWrapper` пула на
//! шрифт и кегль). Память живёт один кадр текста (как кадр `LineLayoutCache`):
//! первый замер следующего кадра начинает с пустой, поэтому холодная загрузка
//! документа ничего не берёт у прошлых документов, а пул обёрток не держит
//! ширин между кадрами. Поколение шрифтов тоже сбрасывает память: добавленный
//! шрифт может сменить подстановку глифов. Внутри кадра память ограничена
//! двумя поколениями по `GENERATION_WORDS` записей; частые слова переходят в
//! текущее.

use crate::Pixels;
use collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Записей в одном поколении. Частые слова текста (служебные, повторы терминов)
/// укладываются в него; редкие уходят со сменой поколения.
const GENERATION_WORDS: usize = 1024;

/// Номер кадра текста: `LineLayoutCache::finish_frame` любого окна его меняет.
static TEXT_FRAME: AtomicUsize = AtomicUsize::new(0);

/// Конец кадра текста: память ширин всех обёрток становится негодной.
pub(crate) fn finish_frame() {
    TEXT_FRAME.fetch_add(1, Ordering::Relaxed);
}

/// Текущий кадр текста.
pub(crate) fn text_frame() -> usize {
    TEXT_FRAME.load(Ordering::Relaxed)
}

#[derive(Default)]
pub(crate) struct WordWidths {
    /// (поколение шрифтов, кадр текста), под которыми записаны ширины.
    stamp: (usize, usize),
    current: HashMap<Box<str>, Pixels>,
    previous: HashMap<Box<str>, Pixels>,
}

impl WordWidths {
    /// Ширина куска `word` в кадре `frame` из памяти или из `measure` (набор платформы).
    pub(crate) fn width(
        &mut self,
        font_generation: usize,
        frame: usize,
        word: &str,
        measure: impl FnOnce() -> Pixels,
    ) -> Pixels {
        let stamp = (font_generation, frame);
        if self.stamp != stamp {
            // Новые таблицы, а не `clear`: прошлый кадр не оставляет ёмкости.
            self.current = HashMap::default();
            self.previous = HashMap::default();
            self.stamp = stamp;
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
    fn a_repeated_word_is_measured_once_per_frame_and_font_generation() {
        let mut widths = WordWidths::default();
        let mut calls = 0;
        let mut width = |widths: &mut WordWidths, generation, frame| {
            widths.width(generation, frame, "word", || {
                calls += 1;
                px(12.5 + generation as f32)
            })
        };
        for _ in 0..3 {
            assert_eq!(width(&mut widths, 0, 0), px(12.5));
        }
        // Новый шрифт в системе: прежние ширины не годятся.
        assert_eq!(width(&mut widths, 1, 0), px(13.5));
        // Следующий кадр меряет заново и не держит прошлых записей.
        assert_eq!(width(&mut widths, 1, 1), px(13.5));
        assert_eq!(widths.len(), 1);
        drop(width);
        assert_eq!(calls, 3);
    }

    #[test]
    fn memory_is_bounded_by_two_generations_and_keeps_frequent_words() {
        let mut widths = WordWidths::default();
        let mut measured = 0;
        for i in 0..(GENERATION_WORDS * 5) {
            widths.width(0, 0, &format!("w{i}"), || {
                measured += 1;
                px(1.)
            });
            // Частое слово между редкими не набирается заново.
            widths.width(0, 0, "the", || {
                measured += 1;
                px(2.)
            });
            assert!(widths.len() <= 2 * GENERATION_WORDS);
        }
        assert_eq!(measured, GENERATION_WORDS * 5 + 1);
    }
}