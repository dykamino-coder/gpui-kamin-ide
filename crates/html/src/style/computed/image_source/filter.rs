//! Resolve typed filter lengths through the shared computed length closure.

use super::{ImageFilter, Len};
use crate::document::state::ValueCx;
use crate::style::values::value::Color;

impl ImageFilter {
    pub(super) fn needs_line_height(&self, vcx: ValueCx<'_>) -> bool {
        self.blur_length
            .into_iter()
            .chain(
                self.shadow_value
                    .iter()
                    .flat_map(|s| s.lengths.into_iter().flatten()),
            )
            .any(|l| l.needs_line_height(vcx))
    }
    pub(super) fn resolve_lengths(&mut self, fix: &impl Fn(&mut Option<Len>), vcx: ValueCx<'_>) {
        fix(&mut self.blur_length);
        if let Some(Len::Px(px)) = self.blur_length {
            if let Some(f) = &mut self.filter {
                f.blur = px.max(0.0);
            }
            self.blur_length = None;
        }
        if let Some(value) = &mut self.shadow_value {
            for length in &mut value.lengths {
                fix(length);
            }
            if let Some(mut shadow) = value.numeric(vcx) {
                shadow.blur *= 2.0;
                self.shadow = Some(shadow);
                self.shadow_value = None;
            }
        }
    }
    pub(super) fn resolve_colors(&mut self, current: Color) {
        if let Some(shadow) = &mut self.shadow
            && self.shadow_current
        {
            shadow.color = current;
        }
        if let Some(value) = &mut self.shadow_value
            && self.shadow_current
        {
            value.color = current;
        }
    }
    pub(crate) fn cacheable(&self) -> bool {
        self.blur_length.is_none() && self.shadow_value.is_none()
    }
}

#[cfg(test)]
mod tests {
    use crate::style::computed::{Computed, ContentItem, ImageSource, Len};
    use crate::style::values::value::Color;

    #[test]
    fn generated_filters_resolve_relative_lengths_once() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut style = Computed {
            font_size: Some(Len::Px(20.0)),
            line_height: Some(Len::Pct(1.5)),
            ..Computed::default()
        };
        style.apply_one(
            "content",
            "filter(url(a.png), drop-shadow(calc(1em + 2px) 1lh 0 green))",
            vcx,
        );
        style.resolve_em(16.0, vcx);
        let Some([ContentItem::Image(ImageSource::Filter { program, .. })]) =
            style.content.as_deref()
        else {
            panic!("typed filter content");
        };
        let shadow = program.shadow.unwrap();
        assert_eq!((shadow.x, shadow.y, shadow.blur), (22.0, 30.0, 0.0));
        assert!(program.cacheable());
        let before = style.content.clone();
        style.resolve_em(80.0, vcx);
        assert_eq!(style.content, before);
    }

    #[test]
    fn viewport_filter_lengths_do_not_enter_the_global_cache_unresolved() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut style = Computed::default();
        style.apply_one(
            "background-image",
            "filter(url(a.png), blur(calc(1em + 2vw)))",
            vcx,
        );
        style.resolve_em(16.0, vcx);
        let Some(ImageSource::Filter { program, .. }) = &style.bg_image else {
            panic!("typed filter");
        };
        assert!(!program.cacheable());
        style.resolve_viewport((500.0, 300.0), vcx);
        let Some(ImageSource::Filter { program, .. }) = &style.bg_image else {
            panic!("typed filter");
        };
        assert_eq!(program.filter.unwrap().blur, 26.0);
        assert!(program.cacheable());
    }

    #[test]
    fn shadow_functions_keep_balanced_color_and_inherited_current_color() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        for color in ["rgb(0, 128, 0)", "currentcolor"] {
            let mut image = ImageSource::parse(
                &format!("filter(url(a.png), drop-shadow(2px 3px 0 {color}))"),
                vcx,
            )
            .unwrap();
            image.resolve_colors(Color::parse("green", vcx).unwrap());
            let ImageSource::Filter { program, .. } = &image else {
                panic!("typed filter");
            };
            assert_eq!(program.calls, 1);
            assert_eq!(
                program.shadow.unwrap().color,
                Color::parse("green", vcx).unwrap()
            );
            image.resolve_colors(Color::parse("red", vcx).unwrap());
            let ImageSource::Filter { program, .. } = &image else {
                panic!("typed filter");
            };
            assert_eq!(
                program.shadow.unwrap().color,
                Color::parse(
                    if color == "currentcolor" {
                        "red"
                    } else {
                        "green"
                    },
                    vcx
                )
                .unwrap()
            );
        }
    }

    #[test]
    fn adjacent_filter_functions_and_quoted_delimiters_stay_separate() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let program = super::ImageFilter::parse(
            "url(\"a)b.svg#mask\")grayscale(.5)invert(.25)",
            doc.values(),
        );
        assert_eq!(program.calls, 3);
        let filter = program.filter.unwrap();
        assert_eq!(filter.grayscale, 0.5);
        assert_eq!(filter.invert, 0.25);
    }
}
