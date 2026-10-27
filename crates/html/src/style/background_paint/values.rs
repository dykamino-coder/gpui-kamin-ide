//! Операнды фона без геометрии элемента и без записи в CSS.

use crate::document::state::ValueCx;
use crate::style::computed::{BgLayer, Computed, Gradient, ImageSource};
use crate::style::values::value::Color;

/// css-backgrounds-3 §2.1: компоненты слоёв и цвет фона, разрешённые на
/// своём элементе. Холст получает эти операнды от донора, обычная коробка —
/// от своего CSS; геометрию обеим передаёт отдельный вход.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BackgroundValues {
    pub color: Option<Color>,
    pub gradient: Option<Gradient>,
    pub gradient_image: Option<ImageSource>,
    pub top: BgLayer,
    pub layers: Option<Vec<BgLayer>>,
    pub color_clip: Option<super::super::computed::BgClip>,
    pub flat: Option<[Color; 3]>,
}

impl BackgroundValues {
    pub(crate) fn from_computed(donor: &Computed) -> Self {
        Self {
            color: donor.background,
            gradient: donor.gradient.clone(),
            gradient_image: donor.gradient_raw.clone(),
            top: BgLayer::from_computed(donor),
            layers: donor.background_layers(),
            color_clip: donor.background_color_clip(),
            flat: None,
        }
    }

    /// Позднее разрешение единиц окна тем же обходом операндов, что у CSS.
    /// Размер области и ось берутся от корня, как до выделения этого входа.
    pub(crate) fn resolve_viewport(
        &mut self,
        root: &Computed,
        viewport: (f32, f32),
        vcx: ValueCx<'_>,
    ) {
        let vp = root.viewport_cx(viewport);
        let fix = |l: &mut Option<crate::style::values::value::Len>| {
            *l = l.map(|l| l.fold_viewport(vp, vcx));
        };
        if let Some(gradient) = &mut self.gradient {
            gradient.resolve_lengths(fix, vcx);
        }
        for image in [&mut self.gradient_image, &mut self.top.image]
            .into_iter()
            .flatten()
        {
            image.resolve_lengths(&fix, vcx);
        }
        for layer in self.layers.iter_mut().flatten() {
            layer.resolve_lengths(&fix, vcx);
        }
    }
}
