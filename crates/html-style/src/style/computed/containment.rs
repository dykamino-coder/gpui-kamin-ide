//! Независимые значения CSS-обособления и их совокупное действие на коробку.

use super::{Computed, ContainerType};

/// Значение `contain`, отдельно от обособления, требуемого другими свойствами.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Containment {
    pub size: bool,
    pub inline_size: bool,
    pub layout: bool,
    pub paint: bool,
    pub style: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContentVisibility {
    #[default]
    Visible,
    Auto,
    Hidden,
}

impl Computed {
    /// CSS Containment 2: auto всегда изолирует layout/style/paint; пропуск
    /// нерелевантного содержимого — отдельное решение, добавляющее size.
    pub(crate) fn refresh_containment(&mut self) {
        let value = self.reset_context.contain_value;
        let hidden = self.reset_generated.content_visibility == ContentVisibility::Hidden;
        let isolated = self.reset_generated.content_visibility != ContentVisibility::Visible;
        self.reset_context.contain_size = Some(
            value.size || self.reset_box_layout.container_type == ContainerType::Size || hidden,
        );
        self.reset_context.contain_inline_size = Some(
            value.inline_size || self.reset_box_layout.container_type == ContainerType::InlineSize,
        );
        self.reset_context.contain_layout = Some(value.layout || isolated);
        self.reset_context.contain_paint = Some(value.paint || isolated);
        self.reset_context.contain_style = Some(
            value.style
                || self.reset_box_layout.container_type != ContainerType::Normal
                || isolated,
        );
        self.reset_context.skip_content = Some(hidden);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declarations(values: &[(&str, &str)]) -> Computed {
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let mut c = Computed::default();
        for (key, value) in values {
            c.apply_one(key, value, doc.values());
        }
        c
    }

    #[test]
    fn auto_contains_even_when_rendering_is_relevant() {
        for values in [
            [("content-visibility", "auto"), ("contain", "none")],
            [("contain", "none"), ("content-visibility", "auto")],
        ] {
            let c = declarations(&values);
            assert_eq!(c.reset_context.contain_layout, Some(true));
            assert_eq!(c.reset_context.contain_style, Some(true));
            assert_eq!(c.reset_context.contain_paint, Some(true));
            assert_eq!(c.reset_context.contain_size, Some(false));
            assert_eq!(c.reset_context.skip_content, Some(false));
            assert!(super::super::box_predicates::establishes_cb(&c));
        }
    }

    #[test]
    fn replacing_visibility_preserves_other_properties() {
        let c = declarations(&[
            ("contain", "paint"),
            ("content-visibility", "hidden"),
            ("content-visibility", "visible"),
        ]);
        assert_eq!(c.reset_context.contain_paint, Some(true));
        assert_eq!(c.reset_context.contain_layout, Some(false));
        assert_eq!(c.reset_context.contain_style, Some(false));
        assert_eq!(c.reset_context.contain_size, Some(false));
        assert_eq!(c.reset_context.skip_content, Some(false));
        let c = declarations(&[
            ("content-visibility", "hidden"),
            ("content-visibility", "auto"),
        ]);
        assert_eq!(c.reset_context.contain_layout, Some(true));
        assert_eq!(c.reset_context.contain_size, Some(false));
        assert_eq!(c.reset_context.skip_content, Some(false));
    }

    #[test]
    fn size_container_and_contain_are_independent() {
        for values in [
            [("container-type", "size"), ("contain", "none")],
            [("contain", "none"), ("container-type", "size")],
        ] {
            let c = declarations(&values);
            assert_eq!(c.reset_context.contain_size, Some(true));
            assert_eq!(c.reset_context.contain_style, Some(true));
            assert_eq!(c.reset_context.contain_layout, Some(false));
        }
        let c = declarations(&[
            ("contain", "layout"),
            ("container-type", "size"),
            ("container-type", "normal"),
        ]);
        assert_eq!(c.reset_context.contain_layout, Some(true));
        assert_eq!(c.reset_context.contain_size, Some(false));
    }

    #[test]
    fn auto_supplies_absolute_fixed_and_clipping_ancestors() {
        use crate::style::cascade::inherit::inherit_unpainted;
        use crate::style::computed::BoxStyle;
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let vcx = doc.values();
        let parent = inherit_unpainted(
            &BoxStyle::default(),
            &declarations(&[("content-visibility", "auto")]),
            vcx,
        );
        for position in ["absolute", "fixed"] {
            let child = inherit_unpainted(&parent, &declarations(&[("position", position)]), vcx);
            assert!(child.cb_ancestor, "{position}");
            assert!(child.reset_transforms.transform_ancestor, "{position}");
            assert!(child.reset_context.fragment_clip_ancestor, "{position}");
        }
    }

    #[test]
    fn css_wide_visibility_keywords_copy_the_property_not_effective_bits() {
        use crate::style::cascade::inherit::inherit_unpainted;
        use crate::style::computed::BoxStyle;
        let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
        let vcx = doc.values();
        let parent = inherit_unpainted(
            &BoxStyle::default(),
            &declarations(&[("content-visibility", "auto")]),
            vcx,
        );
        let child = inherit_unpainted(
            &parent,
            &declarations(&[("content-visibility", "inherit"), ("contain", "size")]),
            vcx,
        );
        assert_eq!(
            child.reset_generated.content_visibility,
            ContentVisibility::Auto
        );
        assert_eq!(child.reset_context.contain_size, Some(true));
        assert_eq!(child.reset_context.contain_paint, Some(true));
        for keyword in ["initial", "unset"] {
            let c = declarations(&[
                ("content-visibility", "auto"),
                ("content-visibility", keyword),
            ]);
            assert_eq!(
                c.reset_generated.content_visibility,
                ContentVisibility::Visible
            );
            assert_eq!(c.reset_context.contain_paint, Some(false));
        }
    }

    #[test]
    fn invalid_visibility_does_not_change_containment() {
        let c = declarations(&[
            ("content-visibility", "auto"),
            ("content-visibility", "invalid"),
        ]);
        assert_eq!(
            c.reset_generated.content_visibility,
            ContentVisibility::Auto
        );
        assert_eq!(c.reset_context.contain_paint, Some(true));
    }
}
