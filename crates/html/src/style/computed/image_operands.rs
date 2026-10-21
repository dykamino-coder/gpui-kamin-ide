//! Обход вычисленных CSS-операндов изображений независимо от маршрута их отрисовки.

use super::{Computed, ImageSource};
impl Computed {
    pub(crate) fn image_operands(&mut self) -> impl Iterator<Item = &mut ImageSource> {
        self.bg_image
            .iter_mut()
            .chain(self.gradient_raw.iter_mut())
            .chain(self.bg_lists.images())
            .chain(
                self.content
                    .iter_mut()
                    .flatten()
                    .filter_map(|item| match item {
                        super::ContentItem::Image(image) => Some(image),
                        _ => None,
                    }),
            )
            .chain(self.mask_image.iter_mut().flatten())
            .chain(self.border_image.iter_mut().filter_map(|b| b.src.as_mut()))
            .chain(self.shape_image.iter_mut())
    }
}
