//! Синхронный клиент таблицы запросов; loader и декодер остаются единственными.
use super::{ImageRequest, ImageState, ImageStore};
use crate::resources::images::types::{DecodeOptions, DecodedImage};
impl ImageStore {
    pub(crate) fn resolve_image(
        &mut self,
        src: &str,
        options: DecodeOptions,
    ) -> Option<(DecodedImage, Option<String>)> {
        let (url, fragment) = self.resolve(src);
        let id = self.request(ImageRequest { url, options });
        self.complete_pending();
        match self.snapshot().state(id)? {
            ImageState::Ready { image, .. } => Some((image.clone(), fragment)),
            ImageState::Pending | ImageState::Failed(_) => None,
        }
    }
}
