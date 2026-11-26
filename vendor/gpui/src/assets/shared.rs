//! Dense immutable BGRA backing shared with an external CPU owner, or produced by it on
//! demand (KaminIDE patch: decode on upload).

use std::{any::Any, borrow::Cow, sync::Arc};

trait Pixels: Any + Send + Sync {
    fn bytes(&self) -> &[u8];
}

impl<T: AsRef<[u8]> + Send + Sync + 'static> Pixels for T {
    fn bytes(&self) -> &[u8] {
        self.as_ref()
    }
}

/// KaminIDE patch: an owner that produces the dense straight BGRA frame when the
/// renderer needs it (an atlas upload), so the image keeps no CPU copy of its pixels.
pub trait LazyBgra: Any + Send + Sync {
    /// The frame, `width * height * 4` bytes; `None` when the source cannot produce it.
    fn bgra(&self) -> Option<Vec<u8>>;
}

enum Backing {
    Ready(Arc<dyn Pixels>),
    Lazy(Arc<dyn LazyBgra>),
}

pub(super) struct SharedImage {
    pub width: u32,
    pub height: u32,
    pixels: Backing,
}

fn frame_len(width: u32, height: u32) -> Option<usize> {
    (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)
}

impl SharedImage {
    pub fn new<T>(width: u32, height: u32, owner: Arc<T>) -> Option<Self>
    where
        T: AsRef<[u8]> + Send + Sync + 'static,
    {
        if owner.as_ref().as_ref().len() != frame_len(width, height)? {
            return None;
        }
        Some(Self {
            width,
            height,
            pixels: Backing::Ready(owner),
        })
    }

    pub fn lazy<T: LazyBgra>(width: u32, height: u32, owner: Arc<T>) -> Option<Self> {
        frame_len(width, height)?;
        Some(Self {
            width,
            height,
            pixels: Backing::Lazy(owner),
        })
    }

    /// Resident pixels; a lazy frame has none until a consumer asks [`Self::upload`].
    pub fn bytes(&self) -> Option<&[u8]> {
        match &self.pixels {
            Backing::Ready(pixels) => Some(pixels.bytes()),
            Backing::Lazy(_) => None,
        }
    }

    /// The frame for one upload: borrowed when resident, produced (and not kept) when lazy.
    pub fn upload(&self) -> Option<Cow<'_, [u8]>> {
        match &self.pixels {
            Backing::Ready(pixels) => Some(Cow::Borrowed(pixels.bytes())),
            Backing::Lazy(owner) => {
                let bytes = owner.bgra()?;
                (Some(bytes.len()) == frame_len(self.width, self.height))
                    .then_some(Cow::Owned(bytes))
            }
        }
    }

    pub fn owner<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        let owner: Arc<dyn Any + Send + Sync> = match &self.pixels {
            Backing::Ready(pixels) => pixels.clone(),
            Backing::Lazy(owner) => owner.clone(),
        };
        owner.downcast().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::LazyBgra;
    use crate::RenderImage;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn shared_pixels_keep_owner_and_validate_frame_extent() {
        let bytes = Arc::new(vec![7, 11, 19, 23]);
        let weak = Arc::downgrade(&bytes);
        let pointer = bytes.as_ptr();
        assert!(RenderImage::from_shared_bgra(2, 1, bytes.clone()).is_none());
        assert!(RenderImage::from_shared_bgra(u32::MAX, u32::MAX, bytes.clone()).is_none());
        let image = RenderImage::from_shared_bgra(1, 1, bytes.clone()).unwrap();
        drop(bytes);
        assert_eq!(image.as_bytes(0).unwrap().as_ptr(), pointer);
        assert_eq!(
            image.shared_bgra_owner::<Vec<u8>>().unwrap().as_ptr(),
            pointer
        );
        assert!(image.shared_bgra_owner::<String>().is_none());
        assert_eq!(image.frame_count(), 1);
        assert_eq!(image.size(0), crate::size(1.into(), 1.into()));
        assert_eq!(image.as_bytes(1), None);
        assert_eq!(image.size(1), Default::default());
        assert!(weak.upgrade().is_some());
        drop(image);
        assert!(weak.upgrade().is_none());
    }

    struct Counted(AtomicUsize, Vec<u8>);

    impl LazyBgra for Counted {
        fn bgra(&self) -> Option<Vec<u8>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Some(self.1.clone())
        }
    }

    #[test]
    fn lazy_pixels_are_produced_per_upload_and_never_resident() {
        let owner = Arc::new(Counted(AtomicUsize::new(0), vec![1, 2, 3, 4, 5, 6, 7, 8]));
        let image = RenderImage::from_lazy_bgra(2, 1, owner.clone()).unwrap();
        assert_eq!(owner.0.load(Ordering::Relaxed), 0, "nothing decoded at creation");
        assert_eq!(image.size(0), crate::size(2.into(), 1.into()));
        assert_eq!(image.as_bytes(0), None, "no resident CPU copy");
        assert_eq!(image.upload_bytes(0).as_deref(), Some(&owner.1[..]));
        assert_eq!(owner.0.load(Ordering::Relaxed), 1);
        assert!(image.shared_bgra_owner::<Counted>().is_some());
        assert!(RenderImage::from_lazy_bgra(3, 1, owner.clone())
            .unwrap()
            .upload_bytes(0)
            .is_none(), "a frame of the wrong extent is refused");
    }
}
