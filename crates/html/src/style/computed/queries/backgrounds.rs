//! Запросы вычисленных слоёв фона и области цвета под нижним слоем.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    /// CSS Backgrounds 3 §3.2: цвет использует clip нижнего слоя изображения;
    /// короткие списки повторяются по кругу, длинные обрезаются (§2.1).
    pub(crate) fn color_clip(&self, _vcx: ValueCx<'_>) -> Option<BgClip> {
        let Some(clips) = self.bg_lists.get(BgListProp::Clip) else {
            return self.bg_clip;
        };
        let Some(images) = self.bg_lists.get(BgListProp::Image) else {
            return self.bg_clip;
        };
        if images.len() < 2 || clips.is_empty() {
            return self.bg_clip;
        }
        clips[(images.len() - 1) % clips.len()].clip
    }

    /// Вычисленные фоновые компоненты, без копии геометрии и CSS элемента.
    pub(crate) fn background_layers(&self) -> Option<Vec<BgLayer>> {
        let images = self.bg_lists.get(BgListProp::Image)?;
        if images.len() < 2 {
            return None;
        }
        Some(
            (0..images.len())
                .map(|i| {
                    let mut layer = BgLayer::from_computed(self);
                    for (prop, layers) in self.bg_lists.iter() {
                        layer.copy_component(prop, &layers[i % layers.len()]);
                    }
                    layer
                })
                .collect(),
        )
    }

    /// Слои сверху вниз. Цвет лежит под всей стопкой; градиенты слоёв идут
    /// общим путём растровых плиток. Разбор CSS завершён каскадом.
    pub(crate) fn bg_layers(&self, _vcx: ValueCx<'_>) -> Option<Vec<Computed>> {
        let layers = self.background_layers()?;
        let mut out = Vec::with_capacity(layers.len());
        for layer in layers {
            let mut c = self.clone();
            c.bg_lists.clear();
            c.background = None;
            for (prop, _) in self.bg_lists.iter() {
                layer.apply(prop, &mut c);
            }
            c.gradient = None;
            c.gradient_raw = None;
            out.push(c);
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_lists_repeat_and_color_uses_the_bottom_clip() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.apply_one("background-image", "url(a), url(b), url(c)", vcx);
        c.apply_one("background-clip", "content-box, padding-box", vcx);
        c.apply_one("background-repeat", "no-repeat, repeat-x", vcx);
        let layers = c.bg_layers(vcx).unwrap();
        assert_eq!(layers.len(), 3);
        assert_eq!(
            layers[0].bg_repeat,
            Some(BgRepeat::Axes(Tiling::None, Tiling::None))
        );
        assert_eq!(layers[1].bg_repeat, Some(BgRepeat::RepeatX));
        assert_eq!(
            layers[2].bg_repeat,
            Some(BgRepeat::Axes(Tiling::None, Tiling::None))
        );
        assert_eq!(c.color_clip(vcx), Some(BgClip::ContentBox));
        assert!(layers.iter().all(|c| c.bg_lists.is_empty()));
    }

    #[test]
    fn font_relative_layers_resolve_before_they_are_queried() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = {
            let mut with_inherited = Computed {
                ..Computed::default()
            };
            with_inherited.font_size = Some(Len::Px(30.0));
            with_inherited
        };
        c.apply_one("background-image", "url(a), url(b)", vcx);
        c.apply_one("background-size", "1em 2em, 3em 4em", vcx);
        c.resolve_em(16.0, vcx);
        let layers = c.bg_layers(vcx).unwrap();
        assert_eq!(
            layers[0].bg_size,
            BgSize::Fixed(Some(Len::Px(30.0)), Some(Len::Px(60.0)))
        );
        assert_eq!(
            layers[1].bg_size,
            BgSize::Fixed(Some(Len::Px(90.0)), Some(Len::Px(120.0)))
        );
    }
}
