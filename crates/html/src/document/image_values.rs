//! Подготовка фактов фона перед style: обход операндов дерева не сворачивает стили,
//! а синхронная загрузка завершается до публикации нового снимка документа.

use super::state::ValueCx;
use crate::dom::Node;
use crate::resources::css_images::facts::BackgroundImages;
use crate::style::computed::{Computed, ImageSource};
use std::collections::HashSet;

pub(crate) fn prepare(nodes: &[Node], vcx: ValueCx<'_>) {
    let mut urls = HashSet::new();
    collect(nodes, &mut urls);
    publish(urls, vcx);
}

pub(super) fn refresh(vcx: ValueCx<'_>) {
    publish(HashSet::new(), vcx);
}

fn publish(urls: HashSet<String>, vcx: ValueCx<'_>) {
    let previous = vcx.doc.background_images.borrow().clone();
    let next = BackgroundImages::prepare(&previous, &mut vcx.doc.images.borrow_mut(), urls);
    *vcx.doc.background_images.borrow_mut() = next;
}

fn collect(nodes: &[Node], urls: &mut HashSet<String>) {
    for node in nodes {
        let Node::Element(e) = node else { continue };
        style(e.style.cascaded(), urls);
        for c in [
            e.hover.as_ref().map(crate::dom::SharedStyle::cascaded),
            e.first_line.as_ref().map(crate::dom::SharedStyle::cascaded),
            e.first_letter
                .as_ref()
                .map(crate::dom::SharedStyle::cascaded),
        ]
        .into_iter()
        .flatten()
        {
            style(c, urls);
        }
        if let Some(anim) = &e.anim {
            for (_, c) in anim.iter() {
                style(c, urls);
            }
        }
        collect(&e.children, urls);
    }
}

fn style(c: &Computed, urls: &mut HashSet<String>) {
    if let Some(Some(image)) = &c.list_style_image {
        operand(image, urls);
    }
    if let Some(image) = &c.bg_image {
        operand(image, urls);
    }
    if let Some(layers) = c.background_layers() {
        for c in layers {
            if let Some(image) = &c.image {
                operand(image, urls);
            }
        }
    }
    for c in [
        c.marker_layer.as_deref(),
        c.first_line.as_deref(),
        c.first_line_own.as_deref(),
        c.first_letter.as_deref(),
        c.first_letter_own.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        style(c, urls);
    }
}

fn operand(image: &ImageSource, urls: &mut HashSet<String>) {
    match image {
        ImageSource::Url(raw) | ImageSource::Fallback { url: raw, .. } => {
            urls.insert(raw.clone());
        }
        ImageSource::Filter { image, .. } => operand(image, urls),
        ImageSource::CrossFade(items) => {
            for (image, _) in items {
                operand(image, urls);
            }
        }
        _ => {}
    }
}

impl ValueCx<'_> {
    /// Только чтение: удержанный снимок не меняется после подготовки следующего кадра.
    pub(crate) fn background_images(
        &self,
    ) -> impl crate::style::computed::flat_background::BackgroundImageValues + use<> {
        self.doc.background_images.borrow().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::state::DocState;
    use crate::resources::images::tests_support::data_png_url;
    use crate::style::computed::flat_background::{BackgroundImageValues, ImageAvailability};
    use crate::style::values::value::Color;
    use std::rc::Rc;

    #[test]
    fn pending_operand_survives_style_until_preparation_and_old_view_stays_pending() {
        let doc = DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let raw = data_png_url();
        let image = ImageSource::Url(raw.clone());
        let mut own = Computed::default();
        own.apply_one("background-image", &format!("url('{raw}')"), vcx);
        own.apply_one("background-clip", "text", vcx);
        let pending = vcx.background_images();
        let revision = doc.images.borrow().snapshot().revision();
        let before = crate::style::cascade::inherit::inherit(&Default::default(), &own, vcx);
        assert_eq!(before.bg_image.as_ref(), Some(&image));
        assert_eq!(doc.images.borrow().snapshot().revision(), revision);
        assert!(doc.images.borrow_mut().take_jobs().is_empty());

        let node = crate::dom::Element::synthetic("div", own.clone(), Default::default(), vec![]);
        prepare(&[Node::Element(node)], vcx);
        let ready = vcx.background_images();
        assert_eq!(
            ready.image_available(&image, true),
            ImageAvailability::Ready
        );
        assert_eq!(
            pending.image_available(&image, true),
            ImageAvailability::Pending
        );
        let revision = doc.images.borrow().snapshot().revision();
        let after = crate::style::cascade::inherit::inherit(&Default::default(), &own, vcx);
        assert!(after.paint_background().top.image.is_none());
        assert_eq!(after.bg_image.as_ref(), Some(&image));
        assert_eq!(
            after.paint_background().flat.map(|v| v[1]),
            ready.uniform_image_color(&image)
        );
        assert_eq!(doc.images.borrow().snapshot().revision(), revision);
        assert_eq!(own.bg_image.as_ref(), Some(&image));
    }

    #[test]
    fn prepare_visits_layers_pseudo_styles_animation_and_nested_operands() {
        let doc = DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let raw = |n| format!("data:,invalid-{n}");
        let with_url = |n| Computed {
            bg_image: Some(ImageSource::Url(raw(n))),
            ..Default::default()
        };
        let mut own = with_url(0);
        own.apply_one(
            "background-image",
            &format!("url('{}'), url('{}')", raw(1), raw(2)),
            vcx,
        );
        own.first_letter_own = Some(Box::new(with_url(3)));
        own.marker_layer = Some(Box::new(with_url(9)));
        let mut node = crate::dom::Element::synthetic("div", own, Default::default(), vec![]);
        node.hover = Some(with_url(4).into());
        node.first_line = Some(with_url(5).into());
        node.anim = Some(Rc::new(vec![(0.0, with_url(6))]));
        let nested = Computed {
            bg_image: Some(ImageSource::CrossFade(vec![
                (
                    ImageSource::Fallback {
                        url: raw(7),
                        color: None,
                    },
                    0.5,
                ),
                (
                    ImageSource::Filter {
                        image: Box::new(ImageSource::Url(raw(8))),
                        program: Default::default(),
                    },
                    0.5,
                ),
            ])),
            ..Default::default()
        };
        node.children
            .push(Node::Element(crate::dom::Element::synthetic(
                "span",
                nested,
                Default::default(),
                vec![],
            )));
        prepare(&[Node::Element(node)], vcx);
        let facts = vcx.background_images();
        for n in 1..=9 {
            assert_eq!(
                facts.image_available(&ImageSource::Url(raw(n)), true),
                ImageAvailability::Failed,
                "operand {n}"
            );
        }
        assert!(doc.images.borrow_mut().take_jobs().is_empty());
    }

    #[test]
    fn frame_refresh_publishes_changed_pixels_without_mutating_a_held_view() {
        use crate::resources::images::tests_support::{TempDir, png_bytes, png_sized};
        let mut dir = TempDir::new("background-frame-refresh");
        let file = dir.write("color.png", &png_bytes([255, 0, 0, 255]));
        let raw = url::Url::from_file_path(file).unwrap().to_string();
        let image = ImageSource::Url(raw);
        let doc = DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let own = Computed {
            bg_image: Some(image.clone()),
            ..Default::default()
        };
        let node = crate::dom::Element::synthetic("div", own, Default::default(), vec![]);
        prepare(&[Node::Element(node)], vcx);
        let red = vcx.background_images();
        dir.write("color.png", &png_sized(2, 1, [0, 0, 255, 255]));
        doc.begin_frame();
        refresh(vcx);
        assert_eq!(red.uniform_image_color(&image), Color::parse("red", vcx));
        assert_eq!(
            vcx.background_images().uniform_image_color(&image),
            Color::parse("blue", vcx)
        );
    }
}
