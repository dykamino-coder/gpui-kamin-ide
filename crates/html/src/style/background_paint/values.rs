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

    /// Операнды одного слоя (css-backgrounds-3 §2.1): цвет лежит под всей
    /// стопкой, а общий градиент принадлежит только единственному слою.
    pub(crate) fn layer_operands(&self, layer: &BgLayer) -> Self {
        Self {
            color: None,
            gradient: self
                .layers
                .is_none()
                .then(|| self.gradient.clone())
                .flatten(),
            gradient_image: None,
            top: layer.clone(),
            layers: None,
            color_clip: layer.clip,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_layer_operands_carry_only_their_own_clip_and_image() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.apply_one("background-color", "red", vcx);
        c.apply_one("background-image", "url(a.png), url(b.png), none", vcx);
        c.apply_one(
            "background-clip",
            "border-area, border-box, content-box",
            vcx,
        );
        let values = BackgroundValues::from_computed(&c);
        let layers = values.layers.clone().unwrap();
        assert_eq!(
            values.color_clip,
            Some(crate::style::computed::BgClip::ContentBox)
        );
        for layer in &layers {
            let own = values.layer_operands(layer);
            assert_eq!(own.color, None);
            assert_eq!(own.gradient, None);
            assert_eq!(own.color_clip, layer.clip);
            assert_eq!(own.top, *layer);
            assert!(own.layers.is_none() && own.flat.is_none());
        }
        let area = Some(crate::style::computed::BgClip::BorderArea);
        assert_eq!(values.top.clip, area);
        assert_ne!(values.layer_operands(&layers[1]).top.clip, area);
    }
}
