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
const DECODER_REVISION: u32 = 2;

/// Результат декодирования, разделяемый документами одного governor.
pub type Decoded = Result<(DecodedImage, ImageMetadata), ImageError>;

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
pub struct DecodedImageCache {
    entries: BoundedCache<Hinted<DecodeKey>, Entry>,
}

impl DecodedImageCache {
    /// Кэш с потолком `budget` байт.
    pub fn new(budget: usize) -> Self {
        Self {
            entries: BoundedCache::new(budget, weigh),
        }
    }

    /// Общий кэш процесса: 128 МиБ, как у прежнего кэша фонов.
    pub fn process() -> Arc<Self> {
        static PROCESS: LazyLock<Arc<DecodedImageCache>> =
            LazyLock::new(|| Arc::new(DecodedImageCache::new(128 * MIB)));
        PROCESS.clone()
    }

    /// Декодировать байты или взять готовое и допустить точки в `budget` документа.
    /// Декодирование идёт без замка кэша; отказ квоты не кэшируется.
    pub fn decode(
        &self,
        bytes: Arc<[u8]>,
        options: DecodeOptions,
        budget: &DocumentBudget,
    ) -> (Decoded, Option<Admission>) {
        // Equivalent orientation requests share their pixels. A real EXIF
        // transform keeps its own cache entry and metadata.
        let options = if options.orientation == super::types::OrientationMode::FromImage
            && !matches!(super::orientation::exif_orientation(&bytes), Some(2..=8))
        {
            DecodeOptions {
                orientation: super::types::OrientationMode::None,
                ..options
            }
        } else {
            options
        };
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
    pub fn len(&self) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::images::{fixtures, types::*};

    #[test]
    fn equivalent_orientations_share_pixels_but_real_transforms_do_not() {
        for tag in [None, Some(1), Some(9), Some(6)] {
            let png = fixtures::png(3, 2);
            let bytes: Arc<[u8]> = tag
                .map_or(png.clone(), |tag| {
                    fixtures::with_chunk(&png, b"eXIf", &fixtures::tiff_orientation(tag))
                })
                .into();
            let cache = DecodedImageCache::new(MIB);
            let budget = crate::budget::RevisionBudget::standalone();
            let options = DecodeOptions {
                colour: ColourPolicy::CssBgra,
                ..Default::default()
            };
            let (turned, _a) = cache.decode(bytes.clone(), options, budget.document());
            let (raw, _b) = cache.decode(
                bytes,
                DecodeOptions {
                    orientation: OrientationMode::None,
                    ..options
                },
                budget.document(),
            );
            let (DecodedImage::Raster(turned), meta) = turned.unwrap() else {
                panic!("raster")
            };
            let (DecodedImage::Raster(raw), _) = raw.unwrap() else {
                panic!("raster")
            };
            let transforms = tag == Some(6);
            assert_eq!(Arc::ptr_eq(&turned, &raw), !transforms, "tag {tag:?}");
            assert_eq!(cache.len(), if transforms { 2 } else { 1 });
            assert_eq!(meta.applied_orientation, tag.filter(|_| transforms));
            assert_eq!((raw.width, raw.height), (3, 2));
            assert_eq!(
                (turned.width, turned.height),
                if transforms { (2, 3) } else { (3, 2) }
            );
        }
    }
}
