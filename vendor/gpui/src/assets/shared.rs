//! Dense immutable BGRA backing shared with an external CPU owner.

use std::{any::Any, sync::Arc};

trait Pixels: Any + Send + Sync {
    fn bytes(&self) -> &[u8];
}

impl<T: AsRef<[u8]> + Send + Sync + 'static> Pixels for T {
    fn bytes(&self) -> &[u8] {
        self.as_ref()
    }
}

pub(super) struct SharedImage {
    pub width: u32,
    pub height: u32,
    pixels: Arc<dyn Pixels>,
}

impl SharedImage {
    pub fn new<T>(width: u32, height: u32, owner: Arc<T>) -> Option<Self>
    where
        T: AsRef<[u8]> + Send + Sync + 'static,
    {
        let len = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        if owner.as_ref().as_ref().len() != len {
            return None;
        }
        Some(Self {
            width,
            height,
            pixels: owner,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        self.pixels.bytes()
    }

    pub fn owner<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        let owner: Arc<dyn Any + Send + Sync> = self.pixels.clone();
        owner.downcast().ok()
    }
}

#[cfg(test)]
mod tests {
    use crate::RenderImage;
    use std::sync::Arc;

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
}
