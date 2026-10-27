//! Процессный кэш декодированных картинок с ключом по СОДЕРЖИМОМУ и параметрам
//! декодирования, а не по адресу: один URL в двух документах с разной базой или политикой —
//! разные запросы, а одинаковые байты под разными адресами — один декодированный растр.
//!
//! Ключ хранит сами байты: равенство сверяет их целиком, хэш служит лишь подсказкой, так
//! что совпадение хэшей двух картинок не подменяет одну другой. Значение неизменяемо
//! (`Arc`), бюджет в байтах учитывает и закодированные байты ключа, и точки; вытесненная
//! запись живёт у держателей. Ошибки декодирования определяются байтами и тоже кэшируются;
//! ошибки чтения, доступа и квот сюда не попадают — они принадлежат запросу документа.
//!
//! Точки растра держат physical квоту host governor (`admission`), поэтому записи разных
//! governor не смешиваются: документ другого домена учёта декодирует свою копию, а не
//! получает чужой буфер без списания. Каждый документ допускает точки логически сам.

use super::admission::{self, Admission};
use super::decode::decode;
use super::types::{DecodeOptions, DecodedImage, ImageError, ImageLimit, ImageMetadata};
use crate::budget::{DocumentBudget, ResourceGovernor};
use crate::cache::{BoundedCache, Hinted, MIB, hint};
use std::sync::{Arc, LazyLock};

/// Версия декодера: меняется вместе с кодеками или порядком шагов декодирования.
const DECODER_REVISION: u32 = 1;

/// Результат декодирования, разделяемый документами одного governor.
pub(crate) type Decoded = Result<(DecodedImage, ImageMetadata), ImageError>;

#[derive(Clone, PartialEq, Eq)]
struct DecodeKey {
    bytes: Arc<[u8]>,
    options: DecodeOptions,
    decoder: u32,
    governor: ResourceGovernor,
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

    /// Декодировать байты или взять готовое и допустить точки в `budget` документа.
    /// Декодирование идёт без замка кэша; отказ квоты не кэшируется.
    pub(crate) fn decode(
        &self,
        bytes: Arc<[u8]>,
        options: DecodeOptions,
        budget: &DocumentBudget,
    ) -> (Decoded, Option<Admission>) {
        let governor = budget.governor().clone();
        let key = Hinted {
            hint: hint((&*bytes, options, DECODER_REVISION, &governor)),
            key: DecodeKey {
                bytes: bytes.clone(),
                options,
                decoder: DECODER_REVISION,
                governor,
            },
        };
        if let Some(hit) = self.entries.get(&key) {
            return admit(hit.decoded, budget);
        }
        let mut admission = None;
        let decoded = decode(&bytes, &options, |declared| {
            let (grant, admitted) = admission::grant(budget, declared)?;
            admission = Some(admitted);
            Ok(grant)
        });
        if !matches!(
            decoded,
            Err(ImageError::LimitExceeded(ImageLimit::Budget(_)))
        ) {
            let encoded = bytes.len();
            let entry = Entry {
                encoded,
                decoded: decoded.clone(),
            };
            self.entries.insert(key, entry);
        }
        (decoded, admission)
    }

    /// Число записей.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

fn admit(decoded: Decoded, budget: &DocumentBudget) -> (Decoded, Option<Admission>) {
    let admission = match &decoded {
        Ok((DecodedImage::Raster(raster), _)) => admission::admit(budget, &raster.retention),
        _ => Ok(None),
    };
    match admission {
        Ok(admission) => (decoded, admission),
        Err(error) => (Err(error), None),
    }
}
