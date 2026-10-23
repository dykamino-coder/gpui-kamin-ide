//! `<svg>` с `<foreignObject>`: внутри — независимый контекст форматирования HTML в порядке
//! покраски SVG. Каждый `foreignObject` раскладывается и снимается в PNG отдельным слоем, слой
//! подставляется в дерево как `<image>`, и весь рисунок растрируется как обычный `<svg>`;
//! единственный вклад с `mix-blend-mode` смешивается с подложкой хоста.
mod geometry;
mod layer;
use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};

pub(super) fn contains(e: &Element) -> bool {
    e.tag.eq_ignore_ascii_case("foreignObject")
        || e.children
            .iter()
            .any(|node| matches!(node, Node::Element(child) if contains(child)))
}

pub(super) fn element(e: &Element, opts: &RenderOpts) -> Option<AnyElement> {
    let vcx = opts.cx.vcx();
    let values = vcx.keep();
    let (w, h) = super::size::size_of(e);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let tree = e.clone();
    let opts = opts.clone();
    let mut wrapper = gpui::div().w(px(w)).h(px(h)).flex_shrink_0();
    if let Some(bg) = e.style.cascaded().background {
        wrapper = wrapper.bg(bg.to_hsla());
    }
    let blend = geometry::sole_blend(e);
    let element = wrapper
        .child(
            gpui::canvas(
                move |_, window, cx| {
                    let mut tree = tree;
                    let viewport = super::size::view_box_ratio(&tree).unwrap_or((w, h));
                    let scale = (w / viewport.0).max(h / viewport.1);
                    let mut layers = Vec::new();
                    layer::prepare(
                        &mut tree,
                        &opts.root_style(),
                        &opts,
                        viewport,
                        scale,
                        window,
                        cx,
                        &mut layers,
                    );
                    (tree, layers)
                },
                move |mut bounds, (mut tree, layers), window, cx| {
                    let vcx = values.vcx();
                    for mut layer in layers {
                        let uri = match layer.cached.take() {
                            Some(uri) => Some(uri),
                            None => window
                                .paint_scene_to_png(layer.size, layer.density, |window| {
                                    if let Some(element) = &mut layer.element {
                                        element.paint(window, cx);
                                    }
                                })
                                .ok()
                                .map(|png| layer::cache(layer.key, &png, vcx)),
                        };
                        if let Some(uri) = uri {
                            layer::set_image(&mut tree, layer.node_id, uri);
                        }
                    }
                    let markup = super::serialize::serialize_sized(&tree, w, h, 0.0, 0.0, vcx);
                    if let Some(image) = super::raster::rasterize(&markup, w, h) {
                        bounds.size = gpui::size(px(w), px(h));
                        super::device_image::paint(window, bounds, image);
                    }
                },
            )
            .absolute()
            .size_full(),
        )
        .into_any_element();
    let style = crate::style::computed::Computed {
        blend,
        ..Default::default()
    };
    Some(crate::paint::effects::grouped::grouped(
        element, &style, vcx,
    ))
}
