//! Набор длинного куска частями (модель словного кэша Gecko и
//! `ReusingTextShaper` Blink): кусок длиннее `MIN * 2` режется на части в
//! точках, заданных самим текстом (пробел между знаками ASCII, хэш слова
//! перед ним), поэтому правка внутри абзаца меняет одну часть, а соседние
//! берутся из памяти документа по ключу своего текста и шрифтов. Резать
//! можно только там, где набор не связывает знаки через пробел: точка
//! принимается, если пара «знак, пробел» и следующий знак, набранные
//! раздельно, дают те же продвижения, что и вместе (кернинг с пробелом,
//! трекинг). Свежий и повторный замер режут одинаково — положение знаков
//! от части не зависит от того, были ли соседи в памяти.

use crate::text::paragraph::*;
use gpui::{LineLayout, Pixels, Window, px};
use std::{ops::Range, sync::Arc};

/// Наименьшая часть: короче — набор целиком дешевле поиска точек.
const MIN: usize = 2048;
/// Наибольшая часть, если хэш слов долго не даёт точки.
const MAX: usize = 8192;
/// Сколько отвергнутых точек терпеть: шрифт, связывающий пробел, режет всё одинаково плохо.
const REJECTS: usize = 16;

impl Paragraph {
    /// Границы частей куска `start..end` (последняя — `end`) или `None`, если
    /// кусок набирается целиком.
    pub(super) fn chunk_ends(
        &self,
        start: usize,
        end: usize,
        window: &mut Window,
    ) -> Option<Vec<usize>> {
        let mut ends = Vec::new();
        if end - start >= MIN * 2 && self.chunkable(start, end) {
            let bytes = self.text.as_bytes();
            let (mut from, mut rejects) = (start, 0);
            let mut c = start + MIN;
            while c + MIN <= end && rejects < REJECTS {
                if candidate(bytes, c)
                    && (word_hash(bytes, c).is_multiple_of(64) || c - from >= MAX)
                {
                    if self.joins_cleanly(c, window) {
                        ends.push(c);
                        from = c;
                        c += MIN;
                        continue;
                    }
                    rejects += 1;
                }
                c += 1;
            }
        }
        (!ends.is_empty()).then(|| {
            ends.push(end);
            ends
        })
    }

    /// Резать можно только письмо без двунаправленности и комбинируемых знаков.
    fn chunkable(&self, start: usize, end: usize) -> bool {
        !self.wrap.rtl
            && self.text[start..end]
                .chars()
                .all(|c| (c as u32) < 0x0590 && !('\u{0300}'..='\u{036F}').contains(&c))
    }

    /// Пара «знак, пробел» и знак после точки `c` набираются раздельно так же, как вместе.
    fn joins_cleanly(&self, c: usize, window: &mut Window) -> bool {
        let whole = self.shape(c - 2..c + 1, window);
        let left = self.shape(c - 2..c, window);
        let right = self.shape(c..c + 1, window);
        let at = whole.x_for_index(2);
        let near = |a: Pixels, b: Pixels| f32::from(a - b).abs() < 1e-3;
        near(at, left.width) && near(whole.width - at, right.width)
    }

    /// Набор отрезка текста абзаца без перевода строки, табуляции и мягкого переноса.
    pub(super) fn shape(&self, r: Range<usize>, window: &mut Window) -> Arc<LineLayout> {
        let runs = self.own_runs(&r);
        window
            .text_system()
            .with_ligature_breaking(false)
            .layout_line_spaced(
                &self.text[r],
                self.font_size,
                &runs,
                None,
                if self.joined_spacing() {
                    px(0.)
                } else {
                    self.letter_spacing
                },
            )
    }

    /// Набор части `r` из памяти документа (или заново, с записью).
    pub(super) fn shape_chunk(
        &self,
        r: Range<usize>,
        keys: &mut Vec<u64>,
        window: &mut Window,
    ) -> Arc<LineLayout> {
        let key = self.chunk_key(&r);
        keys.push(key);
        let hit = self.measures.chunk(key);
        if let Some(Shaped::Chunk(hit)) = hit {
            return hit;
        }
        let layout = self.shape(r, window);
        let chunk = Shaped::Chunk(layout.clone());
        self.measures.put_chunk(key, chunk);
        layout
    }

    /// Запомнить части абзаца с ключом замера `key` (абзац без частей — ничего).
    pub(super) fn remember_chunks(&self, key: u64, chunks: Vec<u64>) {
        if !chunks.is_empty() {
            let list = Shaped::Keys(chunks.into());
            self.measures.put_chunk(list_key(key), list);
        }
    }

    /// Части держатся в памяти, пока держится замер их абзаца.
    pub(super) fn keep_chunks(&self, key: u64) {
        if let Some(Shaped::Keys(keys)) = self.measures.chunk(list_key(key)) {
            keys.iter().for_each(|&k| _ = self.measures.chunk(k));
        }
    }

    /// Ключ набора части: её текст, кегль, трекинг и шрифты прогонов.
    fn chunk_key(&self, r: &Range<usize>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.text[r.clone()].hash(&mut h);
        f32::from(self.font_size).to_bits().hash(&mut h);
        f32::from(self.letter_spacing).to_bits().hash(&mut h);
        self.joined_spacing().hash(&mut h);
        for run in self.own_runs(r) {
            run.len.hash(&mut h);
            run.font_size.map(|s| f32::from(s).to_bits()).hash(&mut h);
            run.font.family.hash(&mut h);
            run.font.weight.0.to_bits().hash(&mut h);
            (run.font.style as u8).hash(&mut h);
            for (tag, value) in run.font.features.tag_value_list() {
                tag.hash(&mut h);
                value.hash(&mut h);
            }
        }
        h.finish()
    }
}

/// Запись памяти частей: набор части или список частей одного замера.
#[derive(Clone)]
pub(in crate::text::paragraph) enum Shaped {
    Chunk(Arc<LineLayout>),
    Keys(std::rc::Rc<[u64]>),
}

/// Ключ списка частей замера `key`: не совпадает с ключами самих частей.
fn list_key(key: u64) -> u64 {
    key.rotate_left(29) ^ 0x9E37_79B9_7F4A_7C15
}

/// Точка `c` — начало слова после пробела, оба соседа пробела ASCII.
fn candidate(bytes: &[u8], c: usize) -> bool {
    bytes[c - 1] == b' ' && bytes[c].is_ascii_alphanumeric() && bytes[c - 2].is_ascii_graphic()
}

/// Хэш восьми байт перед пробелом точки `c`: точки зависят только от текста рядом.
fn word_hash(bytes: &[u8], c: usize) -> u32 {
    bytes[c - 9..c - 1].iter().fold(2_166_136_261u32, |h, &b| {
        (h ^ u32::from(b)).wrapping_mul(16_777_619)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_follow_the_text_not_the_offset() {
        let text = b"alpha beta gamma delta";
        assert!(candidate(text, 6), "after `alpha `");
        assert!(!candidate(text, 5), "the space itself");
        assert!(!candidate(text, 7), "inside a word");
        let shifted = b"xxalpha beta gamma delta";
        assert_eq!(word_hash(text, 11), word_hash(shifted, 13));
    }
}
