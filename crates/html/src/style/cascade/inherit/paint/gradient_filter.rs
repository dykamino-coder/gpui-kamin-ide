//! Наследование, окраска: цветовой `filter` элемента на его единственный
//! градиентный фон. Filter Effects 1 §5: цветовые операции потребляют уже
//! отрисованное изображение, поэтому градиент заворачивается в `filter(...)`
//! как источник `bg_image` с сохранением геометрии, а размытие остаётся на
//! группе поддерева.

use crate::style::computed::{Computed, Filter, ImageFilter, ImageSource};

pub(super) fn apply(
    c: &mut crate::style::background_paint::BackgroundValues,
    own: &Computed,
    f: Filter,
    operations: &[Filter],
) {
    if own.gradient.is_none()
        || c.layers.is_some()
        || operations.iter().all(|op| op.color_matrix().is_none()) && f.color_matrix().is_none()
    {
        return;
    }
    let Some(image) = c.gradient_image.clone() else {
        return;
    };
    set!(
        c.top.image,
        Some(ImageSource::Filter {
            image: Box::new(image),
            program: ImageFilter::color_filter(f, operations),
        })
    );
    set!(c.gradient, None);
    set!(c.gradient_image, None);
}
