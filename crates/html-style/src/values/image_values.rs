//! Подготовка фактов фона перед style: обход операндов дерева не сворачивает стили,
//! а синхронная загрузка завершается до публикации нового снимка документа.

use crate::dom::Node;
use crate::resources::css_images::facts::BackgroundImages;
use crate::style::computed::{Computed, ImageSource};
use crate::values::ValueCx;
use std::collections::HashSet;

pub fn prepare(nodes: &[Node], vcx: ValueCx<'_>) {
    crate::work::add!(FullTreeWalks, 1);
    let mut urls = HashSet::new();
    collect(nodes, &mut urls);
    publish(urls, vcx);
}

pub fn refresh(vcx: ValueCx<'_>) {
    publish(HashSet::new(), vcx);
}

fn publish(urls: HashSet<String>, vcx: ValueCx<'_>) {
    let previous = vcx.doc.background_images.borrow().clone();
    let next = BackgroundImages::prepare(&previous, &mut vcx.doc.images.borrow_mut(), urls);
    *vcx.doc.background_images.borrow_mut() = next;
}

fn collect(nodes: &[Node], urls: &mut HashSet<String>) {
    // Keep only pending siblings: a single-child chain needs no heap frames.
    let mut pending = Vec::new();
    let mut remaining = nodes;
    loop {
        let Some((node, siblings)) = remaining.split_first() else {
            let Some(siblings) = pending.pop() else { break };
            remaining = siblings;
            continue;
        };
        remaining = siblings;
        crate::work::add!(TreeNodesVisited, 1);
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
        if !e.children.is_empty() {
            if !remaining.is_empty() {
                pending.push(remaining);
            }
            remaining = &e.children;
        }
    }
}

#[cfg(test)]
mod depth_tests;

fn style(c: &Computed, urls: &mut HashSet<String>) {
    if let Some(Some(image)) = &c.inh.table_list.list_style_image {
        operand(image, urls);
    }
    if let Some(image) = &c.reset_background.bg_image {
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
        c.reset_generated.marker_layer.as_deref(),
        c.reset_text_box.first_line.as_deref(),
        c.reset_text_box.first_line_own.as_deref(),
        c.reset_text_box.first_letter.as_deref(),
        c.reset_text_box.first_letter_own.as_deref(),
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
    pub fn background_images(
        &self,
    ) -> impl crate::style::computed::flat_background::BackgroundImageValues + use<> {
        self.doc.background_images.borrow().clone()
    }
}

#[cfg(any(test, feature = "engine-test-support"))]
pub mod inventory_cases;
