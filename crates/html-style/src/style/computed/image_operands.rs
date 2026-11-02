//! Обход вычисленных CSS-операндов изображений независимо от маршрута их отрисовки.

use super::{Computed, ImageSource};
impl Computed {
    pub fn image_operands_ref(&self) -> impl Iterator<Item = &ImageSource> {
        let group = &*self.reset_background;
        group
            .bg_image
            .iter()
            .chain(group.gradient_raw.iter())
            .chain(group.bg_lists.images_ref())
            .chain(
                self.reset_generated
                    .content
                    .iter()
                    .flatten()
                    .filter_map(|item| match item {
                        super::ContentItem::Image(image) => Some(image),
                        _ => None,
                    }),
            )
            .chain(self.reset_mask.mask_image.iter().flatten())
            .chain(
                self.reset_border
                    .border_image
                    .iter()
                    .filter_map(|b| b.src.as_ref()),
            )
            .chain(self.reset_shapes.shape_image.iter())
    }

    pub fn image_operands(&mut self) -> impl Iterator<Item = &mut ImageSource> {
        // Пустой обход не меняет CSS-операндов и не должен отделять COW-группы.
        let present = self.image_operands_ref().next().is_some();
        present.then_some(self).into_iter().flat_map(|style| {
            let group = &mut *style.reset_background;
            group
                .bg_image
                .iter_mut()
                .chain(group.gradient_raw.iter_mut())
                .chain(group.bg_lists.images())
                .chain(
                    style
                        .reset_generated
                        .content
                        .iter_mut()
                        .flatten()
                        .filter_map(|item| match item {
                            super::ContentItem::Image(image) => Some(image),
                            _ => None,
                        }),
                )
                .chain(style.reset_mask.mask_image.iter_mut().flatten())
                .chain(
                    style
                        .reset_border
                        .border_image
                        .iter_mut()
                        .filter_map(|b| b.src.as_mut()),
                )
                .chain(style.reset_shapes.shape_image.iter_mut())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_image_walk_preserves_all_group_owners() {
        let base = Computed::default();
        let mut value = base.clone();
        assert_eq!(value.image_operands().count(), 0);
        assert!(base.shares_storage(&value));
    }

    #[test]
    fn read_only_walk_does_not_detach_authored_image_groups() {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let mut base = Computed::default();
        for property in ["background-image", "mask-image", "shape-outside"] {
            base.apply_one(property, "url(image.svg)", doc.values());
        }
        let value = base.clone();
        assert!(value.image_operands_ref().count() >= 3);
        assert!(base.shares_storage(&value));
    }
}
