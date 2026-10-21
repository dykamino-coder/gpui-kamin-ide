//! Наследование, окраска: цветовой `filter` элемента на его единственный
//! градиентный фон. Filter Effects 1 §5: цветовые операции потребляют уже
//! отрисованное изображение, поэтому градиент заворачивается в `filter(...)`
//! как источник `bg_image` с сохранением геометрии, а размытие остаётся на
//! группе поддерева.

use crate::document::state::ValueCx;
use crate::style::computed::{Computed, Filter, ImageFilter, ImageSource};

pub(super) fn apply(c: &mut Computed, own: &Computed, f: Filter, vcx: ValueCx<'_>) {
    if own.gradient.is_none() || c.bg_layers(vcx).is_some() || f.color_matrix().is_none() {
        return;
    }
    let Some(image) = c.gradient_raw.clone() else {
        return;
    };
    c.bg_image = Some(ImageSource::Filter {
        image: Box::new(image),
        program: ImageFilter::color_filter(f),
    });
    c.gradient = None;
    c.gradient_raw = None;
}
