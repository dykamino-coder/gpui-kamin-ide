//! Обход вычисленных CSS-операндов изображений независимо от маршрута их отрисовки.

use super::{Computed, ImageSource};
impl Computed {
    pub fn image_operands(&mut self) -> impl Iterator<Item = &mut ImageSource> {
        let group = &mut *self.reset_background;
        group
            .bg_image
            .iter_mut()
            .chain(group.gradient_raw.iter_mut())
            .chain(group.bg_lists.images())
            .chain(
                self.reset_generated
                    .content
                    .iter_mut()
                    .flatten()
                    .filter_map(|item| match item {
                        super::ContentItem::Image(image) => Some(image),
                        _ => None,
                    }),
            )
            .chain(self.reset_mask.mask_image.iter_mut().flatten())
            .chain(
                self.reset_border
                    .border_image
                    .iter_mut()
                    .filter_map(|b| b.src.as_mut()),
            )
            .chain(self.reset_shapes.shape_image.iter_mut())
    }
}
