//! Application assets and immutable render images, owned or shared with a CPU consumer.

// KaminIDE patch: share immutable CPU backing with the renderer and its consumers.
mod shared;
pub use shared::LazyBgra;
use shared::SharedImage;

use crate::{DevicePixels, Pixels, Result, SharedString, Size, size};
use smallvec::SmallVec;

use image::{Delay, Frame};
use std::{
    borrow::Cow,
    fmt,
    hash::Hash,
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
};

/// A source of assets for this app to use.
pub trait AssetSource: 'static + Send + Sync {
    /// Load the given asset from the source path.
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>>;

    /// List the assets at the given path.
    fn list(&self, path: &str) -> Result<Vec<SharedString>>;
}

impl AssetSource for () {
    fn load(&self, _path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(None)
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

/// A unique identifier for the image cache
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ImageId(pub usize);

#[derive(PartialEq, Eq, Hash, Clone)]
#[expect(missing_docs)]
pub struct RenderImageParams {
    pub image_id: ImageId,
    pub frame_index: usize,
}

/// A cached and processed image, in BGRA format
pub struct RenderImage {
    /// The ID associated with this image
    pub id: ImageId,
    /// The scale factor of this image on render.
    pub(crate) scale_factor: f32,
    data: SmallVec<[Frame; 1]>,
    shared: Option<SharedImage>,
}

impl PartialEq for RenderImage {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for RenderImage {}

impl RenderImage {
    /// Resident CPU blocks exposed by an image, without decoding lazy pixels.
    /// The caller deduplicates addresses shared by scenes and cache owners.
    /// Opaque shared owners' other storage is reported by their own subsystem.
    pub(crate) fn heap_report(&self, sink: &mut crate::heap_report::HeapSink, path: &'static str) {
        crate::heap_report::counted(sink, path, self as *const Self);
        if self.shared.is_some() {
            if let Some(bytes) = self.as_bytes(0) {
                sink(path, bytes.as_ptr(), bytes.len());
            }
        } else {
            for frame in &self.data {
                let bytes = frame.buffer().as_raw();
                sink(path, bytes.as_ptr(), bytes.capacity());
            }
        }
    }

    /// Create a new image from the given data.
    pub fn new(data: impl Into<SmallVec<[Frame; 1]>>) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

        Self {
            id: ImageId(NEXT_ID.fetch_add(1, SeqCst)),
            scale_factor: 1.0,
            data: data.into(),
            shared: None,
        }
    }

    /// Share a dense, straight BGRA frame without copying its pixels. The owner
    /// remains alive with the image, including any resource quota it owns.
    pub fn from_shared_bgra<T>(width: u32, height: u32, owner: std::sync::Arc<T>) -> Option<Self>
    where
        T: AsRef<[u8]> + Send + Sync + 'static,
    {
        let shared = SharedImage::new(width, height, owner)?;
        let mut image = Self::new(SmallVec::new());
        image.shared = Some(shared);
        Some(image)
    }

    /// KaminIDE patch: a dense straight BGRA frame of known size whose pixels the owner
    /// produces for each upload ([`LazyBgra`]); the image keeps no CPU copy.
    pub fn from_lazy_bgra<T: LazyBgra>(width: u32, height: u32, owner: std::sync::Arc<T>) -> Option<Self> {
        let shared = SharedImage::lazy(width, height, owner)?;
        let mut image = Self::new(SmallVec::new());
        image.shared = Some(shared);
        Some(image)
    }

    /// KaminIDE patch: the frame for one upload into a texture: borrowed when resident,
    /// produced on demand (and dropped after the upload) for a lazy image.
    pub fn upload_bytes(&self, frame_index: usize) -> Option<Cow<'_, [u8]>> {
        if let Some(shared) = &self.shared {
            return (frame_index == 0).then(|| shared.upload()).flatten();
        }
        self.as_bytes(frame_index).map(Cow::Borrowed)
    }

    /// Recover a shared CPU owner for a consumer that understands its format.
    /// Owned/animated images have no shared owner.
    pub fn shared_bgra_owner<T: Send + Sync + 'static>(&self) -> Option<std::sync::Arc<T>> {
        self.shared.as_ref()?.owner()
    }

    /// Convert this image into a byte slice.
    pub fn as_bytes(&self, frame_index: usize) -> Option<&[u8]> {
        if let Some(shared) = &self.shared {
            return (frame_index == 0).then(|| shared.bytes()).flatten();
        }
        self.data
            .get(frame_index)
            .map(|frame| frame.buffer().as_raw().as_slice())
    }

    /// Get the size of this image, in pixels.
    pub fn size(&self, frame_index: usize) -> Size<DevicePixels> {
        if let Some(shared) = &self.shared {
            return if frame_index == 0 {
                size(shared.width.into(), shared.height.into())
            } else {
                Size::default()
            };
        }
        self.data
            .get(frame_index)
            .map(|frame| {
                let (width, height) = frame.buffer().dimensions();
                size(width.into(), height.into())
            })
            .unwrap_or_default()
    }

    /// Get the size of this image, in pixels for display, adjusted for the scale factor.
    pub(crate) fn render_size(&self, frame_index: usize) -> Size<Pixels> {
        self.size(frame_index)
            .map(|v| (v.0 as f32 / self.scale_factor).into())
    }

    /// Get the delay of this frame from the previous
    pub fn delay(&self, frame_index: usize) -> Delay {
        self.data
            .get(frame_index)
            .map(|frame| frame.delay())
            .unwrap_or(Delay::from_numer_denom_ms(100, 1))
    }

    /// Get the number of frames for this image.
    pub fn frame_count(&self) -> usize {
        self.shared.as_ref().map_or(self.data.len(), |_| 1)
    }
}

impl fmt::Debug for RenderImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageData")
            .field("id", &self.id)
            .field("size", &self.size(0))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::SmallVec;

    #[test]
    fn empty_render_image_does_not_panic() {
        let image = RenderImage::new(SmallVec::new());
        assert_eq!(image.frame_count(), 0);
        assert_eq!(image.size(0), Size::default());
        assert_eq!(image.as_bytes(0), None);
        assert_eq!(image.render_size(0), Size::default());
        assert_eq!(image.delay(0), Delay::from_numer_denom_ms(100, 1));
        let _ = format!("{image:?}");
    }
}
