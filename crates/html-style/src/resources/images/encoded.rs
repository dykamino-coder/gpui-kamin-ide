//! A raster known by its file header: its size after EXIF rotation and its pixel quota
//! exist from load, while the pixels are decoded on demand by a consumer (the GPU
//! upload, a CPU filter or mask, future script: `drawImage`; `naturalWidth` reads the
//! size without pixels) and the document does not keep them.
//!
//! This is how the leanest engines hold image memory: cc (`GpuImageDecodeCache`) locks
//! CPU pixels only until the GPU upload, Gecko (`SurfaceCache`) decodes at paint and
//! discards unlocked surfaces; Servo and Blitz keep the decoded raster of every loaded
//! image for the document's lifetime. Here the pixels of a VISIBLE image live in the
//! texture, a CPU copy only with whoever holds it now (`Weak`), plus a small shared LRU
//! of recent decodes so that the paint of neighbouring frames does not decode again.

use super::admission::Retention;
use super::decode::{decode_pixels, decode_vec};
use super::types::{AlphaMode, CpuRaster, DecodeOptions, PixelFormat};
use crate::cache::{BoundedCache, MIB};
use std::any::Any;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

/// The file bytes of a raster with its header facts; pixels: [`EncodedRaster::pixels`].
pub struct EncodedRaster {
    pub width: u32,
    pub height: u32,
    bytes: Arc<[u8]>,
    options: DecodeOptions,
    retention: Retention,
    /// The pixels, while at least one consumer holds them.
    live: Mutex<Weak<CpuRaster>>,
    /// What the painter made of this file for the GPU (its texture image): it lives as
    /// long as the raster, so the upload dies with the documents and caches holding the
    /// file, and no process cache keeps a texture of a file nobody holds.
    upload: OnceLock<Arc<dyn Any + Send + Sync>>,
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

/// The LRU key is the raster's address: equal bytes are already merged by the decode cache.
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

/// Recently decoded pixels of the process: a budget for a few large images, not for all
/// images of a document.
/// An entry goes once nobody else holds its file (a closed document's raster).
static RECENT: BoundedCache<Recent, Option<Arc<CpuRaster>>> =
    BoundedCache::new(16 * MIB, super::pixels::optional_bytes)
        .dies_with(|key| Arc::strong_count(&key.0) > 1);

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
            upload: OnceLock::new(),
        }
    }

    /// The consumer's value derived from this raster, made once by `make` (the paint's
    /// texture image; `T` is opaque here). The value must hold the raster only weakly,
    /// or the two would keep each other alive.
    pub fn upload<T: Any + Send + Sync>(&self, make: impl FnOnce() -> T) -> Option<Arc<T>> {
        let value = self
            .upload
            .get_or_init(|| Arc::new(make()) as Arc<dyn Any + Send + Sync>);
        value.clone().downcast().ok()
    }

    /// The physical pixel quota admitted at load.
    pub fn retention(&self) -> &Retention {
        &self.retention
    }

    /// The file bytes (held for the request's lifetime).
    pub fn encoded_len(&self) -> usize {
        self.bytes.len()
    }

    /// The pixels: held by someone, recent, or decoded now. Synchronous, like Blink's
    /// decode for `drawImage`; `None` when the data is corrupt.
    pub fn pixels(self: &Arc<Self>) -> Option<Arc<CpuRaster>> {
        if let Some(held) = self.held() {
            return Some(held);
        }
        let raster = RECENT.get_or_insert_with(Recent(self.clone()), || self.decode_now())?;
        *self.live.lock().unwrap_or_else(PoisonError::into_inner) = Arc::downgrade(&raster);
        Some(raster)
    }

    /// The pixels without an entry in the shared LRU: for a one-off read (a texture
    /// upload, a solid colour check) after which no CPU copy is needed.
    pub fn pixels_once(&self) -> Option<Arc<CpuRaster>> {
        self.held().or_else(|| self.decode_now())
    }

    /// Dense straight BGRA pixels for one texture upload, in a buffer of their own: a
    /// held raster is copied, a decode hands over the codec's buffer as it is (no shared
    /// raster in between, so an upload holds one image of pixels, not three).
    pub fn bgra_once(&self) -> Option<Vec<u8>> {
        if let Some(held) = self.held() {
            let dense = held.format == PixelFormat::Bgra8
                && held.alpha == AlphaMode::Straight
                && held.stride == held.width as usize * 4;
            return Some(match dense {
                true => held.bytes.to_vec(),
                false => super::pixels::straight_bgra(&held),
            });
        }
        crate::work::add!(ImageDecodes, 1);
        let decoded = decode_vec(&self.bytes, &self.options).ok()?;
        if (decoded.width, decoded.height) != (self.width, self.height) {
            return None;
        }
        let mut pixels = decoded.pixels;
        if decoded.format == PixelFormat::Rgba8 {
            pixels.chunks_exact_mut(4).for_each(|p| p.swap(0, 2));
        }
        Some(pixels)
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
        // Corrupt pixels behind an intact header: the sides must match the declared ones.
        ((raster.width, raster.height) == (self.width, self.height)).then(|| Arc::new(raster))
    }
}

/// The raster of an image: ready pixels (procedural, filtered, test) or a file known by
/// its header whose pixels are decoded on demand ([`EncodedRaster`]).
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
    /// The raster's size in pixels after orientation; needs no pixels.
    pub fn size(&self) -> (u32, u32) {
        match self {
            Self::Ready(r) => (r.width, r.height),
            Self::Encoded(e) => (e.width, e.height),
        }
    }

    /// The raster's pixels (decoded on demand; `None` when the data is corrupt).
    pub fn pixels(&self) -> Option<Arc<CpuRaster>> {
        match self {
            Self::Ready(r) => Some(r.clone()),
            Self::Encoded(e) => e.pixels(),
        }
    }

    /// The raster's physical pixel quota.
    pub fn retention(&self) -> &Retention {
        match self {
            Self::Ready(r) => &r.retention,
            Self::Encoded(e) => e.retention(),
        }
    }

    /// The same value, not an equal one.
    pub fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ready(a), Self::Ready(b)) => Arc::ptr_eq(a, b),
            (Self::Encoded(a), Self::Encoded(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}
