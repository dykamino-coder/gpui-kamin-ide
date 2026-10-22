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
        let mut probe = Computed::default();
        clips[(images.len() - 1) % clips.len()].apply(BgListProp::Clip, &mut probe);
        probe.bg_clip
    }

    /// Clip of the colour that needs an inset layer (`paint::box_tree::styled::clip_layer`):
    /// `padding-box`/`content-box` whose edge coincides with the border edge (no border, and
    /// for `content-box` no padding) paints exactly the border box (css-backgrounds-3 §3.7),
    /// so the box keeps painting its colour itself. Boxes without a clip layer — replaced
    /// elements such as `<canvas>` — otherwise lost the colour of `background: green
    /// content-box` (CSS2 floats-clear/clear-on-replaced-element).
    pub(crate) fn color_inset_clip(&self, vcx: ValueCx<'_>) -> Option<BgClip> {
        let clip = self.color_clip(vcx)?;
        let zero = |l: Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
        let b = self.borders();
        let p = &self.padding;
        let flush = match clip {
            BgClip::PaddingBox => [b.top, b.right, b.bottom, b.left].into_iter().all(zero),
            BgClip::ContentBox => [b.top, b.right, b.bottom, b.left, p.top, p.right, p.bottom]
                .into_iter()
                .chain([p.left])
                .all(zero),
            _ => false,
        };
        (!flush).then_some(clip)
    }

    /// Слои сверху вниз. Цвет лежит под всей стопкой; градиенты слоёв идут
    /// общим путём растровых плиток. Разбор CSS завершён каскадом.
    pub(crate) fn bg_layers(&self, _vcx: ValueCx<'_>) -> Option<Vec<Computed>> {
        let images = self.bg_lists.get(BgListProp::Image)?;
        if images.len() < 2 {
            return None;
        }
        let mut out = Vec::with_capacity(images.len());
        for i in 0..images.len() {
            let mut c = self.clone();
            c.bg_lists.clear();
            c.background = None;
            for (prop, layers) in self.bg_lists.iter() {
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
    fn inset_clip_flush_with_the_border_edge_paints_on_the_box() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        // `<canvas style="background: green content-box">`: no border, no padding.
        let mut c = Computed::default();
        c.apply_one("background-clip", "content-box", vcx);
        assert_eq!(c.color_clip(vcx), Some(BgClip::ContentBox));
        assert_eq!(c.color_inset_clip(vcx), None);
        c.padding.bottom = Some(Len::Px(10.0));
        assert_eq!(c.color_inset_clip(vcx), Some(BgClip::ContentBox));
        let mut p = Computed::default();
        p.apply_one("background-clip", "padding-box", vcx);
        p.padding.top = Some(Len::Px(4.0));
        assert_eq!(p.color_inset_clip(vcx), None);
        p.border_width.left = Some(Len::Px(2.0));
        p.border_visible[3] = Some(true);
        assert_eq!(p.color_inset_clip(vcx), Some(BgClip::PaddingBox));
        let mut t = Computed::default();
        t.apply_one("background-clip", "text", vcx);
        assert_eq!(t.color_inset_clip(vcx), Some(BgClip::Text));
    }

    #[test]
    fn font_relative_layers_resolve_before_they_are_queried() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed {
            font_size: Some(Len::Px(30.0)),
            ..Computed::default()
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
