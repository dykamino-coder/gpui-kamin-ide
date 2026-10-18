//! Запросы вычисленных слоёв фона и области цвета под нижним слоем.

use super::*;

impl Computed {
    /// CSS Backgrounds 3 §3.2: цвет использует clip нижнего слоя изображения;
    /// короткие списки повторяются по кругу, длинные обрезаются (§2.1).
    pub(crate) fn color_clip(&self) -> Option<BgClip> {
        let Some(clips) = self.bg_lists.get(BgListProp::Clip) else {
            return self.bg_clip;
        };
        let Some(images) = self.bg_lists.get(BgListProp::Image) else {
            return self.bg_clip;
        };
        if images.len() < 2 || clips.is_empty() {
            return self.bg_clip;
        }
        let mut probe = Computed::default();
        clips[(images.len() - 1) % clips.len()].apply(BgListProp::Clip, &mut probe);
        probe.bg_clip
    }

    /// Слои сверху вниз. Цвет лежит под всей стопкой; градиенты слоёв идут
    /// общим путём растровых плиток. Разбор CSS завершён каскадом.
    pub(crate) fn bg_layers(&self) -> Option<Vec<Computed>> {
        let images = self
            .bg_lists
            .get(BgListProp::Image)
            .or_else(|| self.bg_lists.get(BgListProp::Shorthand))?;
        if images.len() < 2 {
            return None;
        }
        let mut out = Vec::with_capacity(images.len());
        for i in 0..images.len() {
            let mut c = self.clone();
            c.bg_lists.clear();
            c.background = None;
            if let Some(layers) = self.bg_lists.get(BgListProp::Shorthand) {
                layers[i % layers.len()].apply(BgListProp::Shorthand, &mut c);
            }
            for (prop, layers) in self
                .bg_lists
                .iter()
                .filter(|(k, _)| *k != BgListProp::Shorthand)
            {
                layers[i % layers.len()].apply(prop, &mut c);
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
        let mut c = Computed::default();
        c.apply_one("background-image", "url(a), url(b), url(c)");
        c.apply_one("background-clip", "content-box, padding-box");
        c.apply_one("background-repeat", "no-repeat, repeat-x");
        let layers = c.bg_layers().unwrap();
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
        assert_eq!(c.color_clip(), Some(BgClip::ContentBox));
        assert!(layers.iter().all(|c| c.bg_lists.is_empty()));
    }

    #[test]
    fn font_relative_layers_resolve_before_they_are_queried() {
        let mut c = Computed {
            font_size: Some(Len::Px(30.0)),
            ..Computed::default()
        };
        c.apply_one("background-image", "url(a), url(b)");
        c.apply_one("background-size", "1em 2em, 3em 4em");
        c.resolve_em(16.0);
        let layers = c.bg_layers().unwrap();
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
