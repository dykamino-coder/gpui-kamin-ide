//! Наследование, окраска: цветовой `filter` элемента на его единственный
//! градиентный фон. Filter Effects 1 §5: цветовые операции потребляют уже
//! отрисованное изображение, поэтому градиент заворачивается в `filter(...)`
//! как источник `bg_image` с сохранением геометрии, а размытие остаётся на
//! группе поддерева.

use crate::document::state::ValueCx;
use crate::style::computed::{BoxStyle, Computed, Filter, ImageFilter, ImageSource};

pub(super) fn apply(c: &mut BoxStyle, own: &Computed, f: Filter, vcx: ValueCx<'_>) {
    if own.gradient.is_none() || c.bg_layers(vcx).is_some() || f.color_matrix().is_none() {
        return;
    }
    let Some(image) = c.gradient_raw.clone() else {
        return;
    };
    set!(
        c.bg_image,
        Some(ImageSource::Filter {
            image: Box::new(image),
            program: ImageFilter::color_filter(f),
        })
    );
    set!(c.gradient, None);
    set!(c.gradient_raw, None);
}
