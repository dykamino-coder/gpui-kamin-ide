//! Картинка маркера: сервис изображений документа и default sizing из
//! css-lists-3 §image-markers / css-images-3 §default-sizing.

use crate::document::state::ValueCx;
use crate::dom::Node;
use crate::layout::table::anon::anon_element;
use crate::resources::{css_images::computed_source, images::natural::default_size};
use crate::style::computed::{Computed, ContentItem};
use crate::style::values::value::Len;

pub(super) fn resolve(
    item: &Computed,
    marker: Option<&Computed>,
    vcx: ValueCx<'_>,
) -> Option<Node> {
    let image = item.list_style_image.as_ref()?.as_ref()?;
    let orient = item.image_orient_none != Some(true);
    let source = computed_source(image, orient, vcx)?;
    let em = marker.unwrap_or(item).font_cx(vcx).font_px;
    let (w, h) = default_size(source.intrinsic(), (em, em));
    let mut node = anon_element("img", vec![]);
    node.inline = true;
    let used = node.style.used_mut();
    used.content = Some(vec![ContentItem::Image(image.clone())]);
    used.width = Some(Len::Px(w));
    used.height = Some(Len::Px(h));
    used.image_orient_none = item.image_orient_none;
    used.image_pixelated = item.image_pixelated;
    Some(Node::Element(node))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{image_values, state::DocState};
    use crate::dom::Element;
    use crate::style::cascade::inherit::inherit;
    use crate::style::computed::BoxStyle;
    use crate::style::css::parse_decls;

    fn item(css: &str, vcx: ValueCx<'_>) -> Element {
        let mut style = Computed::default();
        style.apply_decls_with_vars(&parse_decls(css), &Default::default(), vcx);
        let mut item = Element::synthetic("li", style, Default::default(), vec![]);
        item.list_item = Some(3);
        item
    }

    fn dimensions(node: &Node) -> (Option<Len>, Option<Len>) {
        let Node::Element(e) = node else {
            panic!("image element")
        };
        assert_eq!(e.tag, "img");
        assert!(e.inline && e.style.used().content_image().is_some());
        (e.style.used().width, e.style.used().height)
    }

    #[test]
    fn marker_image_default_size_and_natural_ratio() {
        let doc = DocState::new(Default::default());
        let vcx = doc.values();
        for (image, expected) in [
            ("linear-gradient(green, green)", (20., 20.)),
            (
                "url('data:image/svg+xml,<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 2 1\"/>')",
                (20., 10.),
            ),
            (
                "url('data:image/svg+xml,<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"30\" height=\"10\"/>')",
                (30., 10.),
            ),
        ] {
            let li = item(&format!("font-size:20px; list-style-image:{image}"), vcx);
            let merged = inherit(&BoxStyle::default(), &li.style.used(), vcx);
            let image = resolve(&merged, None, vcx).unwrap();
            assert_eq!(
                dimensions(&image),
                (Some(Len::Px(expected.0)), Some(Len::Px(expected.1)))
            );
        }
    }

    #[test]
    fn marker_image_failure_falls_back_and_type_none_does_not_hide_image() {
        let doc = DocState::new(Default::default());
        let vcx = doc.values();
        for position in ["inside", "outside"] {
            let li = item(
                &format!(
                    "list-style:decimal {position}; list-style-image:url('data:text/plain,broken')"
                ),
                vcx,
            );
            let merged = inherit(&BoxStyle::default(), &li.style.used(), vcx);
            let marker = super::super::resolve(&li, &Computed::default(), &merged, vcx).unwrap();
            assert!(marker.image.is_none());
            assert_eq!(marker.text, "3. ");
            let li = item(
                &format!(
                    "list-style:none {position}; list-style-image:linear-gradient(green, green)"
                ),
                vcx,
            );
            let merged = inherit(&BoxStyle::default(), &li.style.used(), vcx);
            let marker = super::super::resolve(&li, &Computed::default(), &merged, vcx).unwrap();
            let Node::Element(span) = super::super::node(&li, &merged, marker) else {
                panic!()
            };
            assert_eq!(span.children.len(), 2);
            dimensions(&span.children[0]);
            assert!(matches!(&span.children[1], Node::Text(s) if s == " "));
        }
    }

    #[test]
    fn inherited_marker_image_is_prepared_and_explicit_content_takes_precedence() {
        let doc = DocState::new(Default::default());
        let vcx = doc.values();
        let raw = crate::resources::images::tests_support::data_png_url();
        let parent = item(&format!("list-style-image:url('{raw}')"), vcx);
        image_values::prepare(&[Node::Element(parent.clone())], vcx);
        use crate::style::computed::ImageSource;
        use crate::style::computed::flat_background::{BackgroundImageValues, ImageAvailability};
        assert_eq!(
            vcx.background_images()
                .image_available(&ImageSource::Url(raw), true),
            ImageAvailability::Ready
        );
        assert!(doc.images.borrow_mut().take_jobs().is_empty());
        let mut li = item("list-style-type:none", vcx);
        let merged = inherit(
            &BoxStyle::shared(parent.style.used()),
            &li.style.used(),
            vcx,
        );
        assert!(
            super::super::resolve(&li, &parent.style.used(), &merged, vcx)
                .unwrap()
                .image
                .is_some()
        );
        let mut layer = Computed::default();
        layer.apply_one("content", "none", vcx);
        li.style.cascaded_mut().marker_layer = Some(Box::new(layer));
        li.style.cascaded_mut().no_marker = Some(true);
        // Как в dom/walk: содержимое псевдоэлемента задаётся до наследования.
        // После смены деклараций прежний снимок merged остаётся неизменным.
        let merged = inherit(
            &BoxStyle::shared(parent.style.used()),
            li.style.cascaded(),
            vcx,
        );
        assert!(super::super::resolve(&li, &parent.style.used(), &merged, vcx).is_none());
        li.style
            .cascaded_mut()
            .marker_layer
            .as_mut()
            .unwrap()
            .apply_one("content", "'custom'", vcx);
        li.style.cascaded_mut().no_marker = Some(false);
        li.style.cascaded_mut().marker_text = Some("custom".into());
        let merged = inherit(
            &BoxStyle::shared(parent.style.used()),
            li.style.cascaded(),
            vcx,
        );
        let marker = super::super::resolve(&li, &parent.style.used(), &merged, vcx).unwrap();
        assert!(marker.image.is_none());
        assert_eq!(marker.text, "custom");
    }
}
