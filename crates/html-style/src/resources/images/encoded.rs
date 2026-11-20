//! Растр, известный по заголовку файла: стороны после EXIF-поворота и квота точек есть с
//! загрузки, а сами точки декодируются по требованию потребителя — загрузки в
//! графический адаптер, CPU-фильтра или маски, будущего скрипта (`drawImage`,
//! `naturalWidth` читает стороны без точек) — и документом не удерживаются.
//!
//! Так держат память лучшие движки: cc (`GpuImageDecodeCache`) запирает CPU-точки лишь
//! до загрузки в GPU, Gecko (`SurfaceCache`) декодирует при отрисовке и выбрасывает
//! незапертое; Servo и Blitz хранят декодированный растр каждой загруженной картинки
//! всё время жизни документа. Здесь точки ВИДИМОЙ картинки живут в текстуре, CPU-копия —
//! только у тех, кто её сейчас держит (`Weak`), плюс небольшой общий LRU недавно
//! декодированного, чтобы покраска соседних кадров не декодировала повторно.

use super::admission::Retention;
use super::decode::decode_pixels;
use super::types::{CpuRaster, DecodeOptions};
use crate::cache::{BoundedCache, MIB};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, PoisonError, Weak};

/// Байты файла растра с фактами заголовка; точки — [`EncodedRaster::pixels`].
pub struct EncodedRaster {
    pub width: u32,
    pub height: u32,
    bytes: Arc<[u8]>,
    options: DecodeOptions,
    retention: Retention,
    /// Точки, пока их держит хоть один потребитель.
    live: Mutex<Weak<CpuRaster>>,
}

impl std::fmt::Debug for EncodedRaster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncodedRaster")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// Ключ LRU — сам растр по адресу: одинаковые байты уже слиты кэшем декодирования.
#[derive(Clone)]
struct Recent(Arc<EncodedRaster>);

impl PartialEq for Recent {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Recent {}

impl Hash for Recent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

/// Недавно декодированные точки процесса: бюджет на несколько крупных картинок, а не на
/// все картинки документа.
static RECENT: BoundedCache<Recent, Option<Arc<CpuRaster>>> =
    BoundedCache::new(16 * MIB, super::pixels::optional_bytes);

impl EncodedRaster {
    pub(super) fn new(
        bytes: Arc<[u8]>,
        options: DecodeOptions,
        (width, height): (u32, u32),
        retention: Retention,
    ) -> Self {
        Self {
            width,
            height,
            bytes,
            options,
            retention,
            live: Mutex::new(Weak::new()),
        }
    }

    /// Physical квота точек, допущенная при загрузке.
    pub fn retention(&self) -> &Retention {
        &self.retention
    }

    /// Байты файла (держатся всё время жизни запроса).
    pub fn encoded_len(&self) -> usize {
        self.bytes.len()
    }

    /// Точки растра: удерживаемые кем-то, недавние или декодированные сейчас.
    /// Синхронно, как декодирование Blink для `drawImage`; `None` — данные повреждены.
    pub fn pixels(self: &Arc<Self>) -> Option<Arc<CpuRaster>> {
        if let Some(held) = self.held() {
            return Some(held);
        }
        let raster = RECENT.get_or_insert_with(Recent(self.clone()), || self.decode_now())?;
        *self.live.lock().unwrap_or_else(PoisonError::into_inner) = Arc::downgrade(&raster);
        Some(raster)
    }

    /// Точки без записи в общий LRU: для однократного чтения (загрузка в текстуру,
    /// проверка сплошного цвета), после которого CPU-копия не нужна.
    pub fn pixels_once(&self) -> Option<Arc<CpuRaster>> {
        self.held().or_else(|| self.decode_now())
    }

    fn held(&self) -> Option<Arc<CpuRaster>> {
        self.live
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .upgrade()
    }

    fn decode_now(&self) -> Option<Arc<CpuRaster>> {
        crate::work::add!(ImageDecodes, 1);
        let raster = decode_pixels(&self.bytes, &self.options, self.retention.clone()).ok()?;
        // Повреждённые точки при целом заголовке: стороны обязаны совпасть с заявленными.
        ((raster.width, raster.height) == (self.width, self.height)).then(|| Arc::new(raster))
    }
}

/// Растр картинки: готовые точки (процедурный, отфильтрованный, тестовый) или файл,
/// известный по заголовку, чьи точки декодируются по требованию ([`EncodedRaster`]).
#[derive(Clone, Debug)]
pub enum RasterImage {
    Ready(Arc<CpuRaster>),
    Encoded(Arc<EncodedRaster>),
}

impl From<Arc<CpuRaster>> for RasterImage {
    fn from(raster: Arc<CpuRaster>) -> Self {
        Self::Ready(raster)
    }
}

impl RasterImage {
    /// Стороны растра в точках после ориентации; точек не требует.
    pub fn size(&self) -> (u32, u32) {
        match self {
            Self::Ready(r) => (r.width, r.height),
            Self::Encoded(e) => (e.width, e.height),
        }
    }

    /// Точки растра (декодирование по требованию; `None` — данные повреждены).
    pub fn pixels(&self) -> Option<Arc<CpuRaster>> {
        match self {
            Self::Ready(r) => Some(r.clone()),
            Self::Encoded(e) => e.pixels(),
        }
    }

    /// Physical квота точек растра.
    pub fn retention(&self) -> &Retention {
        match self {
            Self::Ready(r) => &r.retention,
            Self::Encoded(e) => e.retention(),
        }
    }

    /// Одно и то же значение, а не равное.
    pub fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ready(a), Self::Ready(b)) => Arc::ptr_eq(a, b),
            (Self::Encoded(a), Self::Encoded(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}
