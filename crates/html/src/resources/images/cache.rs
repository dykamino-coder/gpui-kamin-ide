//! Процессный кэш декодированных картинок с ключом по СОДЕРЖИМОМУ и параметрам
//! декодирования, а не по адресу: один URL в двух документах с разной базой или политикой —
//! разные запросы, а одинаковые байты под разными адресами — один декодированный растр.
//!
//! Ключ хранит сами байты: равенство сверяет их целиком, хэш служит лишь подсказкой, так
//! что совпадение хэшей двух картинок не подменяет одну другой. Значение неизменяемо
//! (`Arc`), бюджет в байтах учитывает и закодированные байты ключа, и точки; вытесненная
//! запись живёт у держателей. Ошибки декодирования определяются байтами и тоже кэшируются;
//! ошибки чтения и доступа сюда не попадают — они принадлежат запросу документа.

use super::decode::decode;
use super::types::{DecodeOptions, DecodedImage, ImageError, ImageMetadata};
use crate::cache::{BoundedCache, Hinted, MIB, hint};
use std::sync::{Arc, LazyLock};

/// Версия декодера: меняется вместе с кодеками или порядком шагов декодирования.
const DECODER_REVISION: u32 = 1;

/// Результат декодирования, разделяемый документами.
pub(crate) type Decoded = Result<(DecodedImage, ImageMetadata), ImageError>;

#[derive(Clone, PartialEq, Eq)]
struct DecodeKey {
    bytes: Arc<[u8]>,
    options: DecodeOptions,
    decoder: u32,
}

#[derive(Clone)]
struct Entry {
    encoded: usize,
    decoded: Decoded,
}

fn weigh(entry: &Entry) -> usize {
    let decoded = entry
        .decoded
        .as_ref()
        .map_or(0, |(image, _)| image.held_bytes());
    entry.encoded + decoded
}

/// Ограниченный кэш декодированных картинок; внедряется в хранилища документов.
pub(crate) struct DecodedImageCache {
    entries: BoundedCache<Hinted<DecodeKey>, Entry>,
}

impl DecodedImageCache {
    /// Кэш с потолком `budget` байт.
    pub(crate) fn new(budget: usize) -> Self {
        Self {
            entries: BoundedCache::new(budget, weigh),
        }
    }

    /// Общий кэш процесса: 128 МиБ, как у прежнего кэша фонов.
    pub(crate) fn process() -> Arc<Self> {
        static PROCESS: LazyLock<Arc<DecodedImageCache>> =
            LazyLock::new(|| Arc::new(DecodedImageCache::new(128 * MIB)));
        PROCESS.clone()
    }

    /// Декодировать байты или взять готовое; декодирование идёт без замка кэша.
    pub(crate) fn decode(&self, bytes: Arc<[u8]>, options: DecodeOptions) -> Decoded {
        let key = Hinted {
            hint: hint((&*bytes, options, DECODER_REVISION)),
            key: DecodeKey {
                bytes: bytes.clone(),
                options,
                decoder: DECODER_REVISION,
            },
        };
        self.entries
            .get_or_insert_with(key, || Entry {
                encoded: bytes.len(),
                decoded: decode(&bytes, &options),
            })
            .decoded
    }

    /// Число записей.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}
