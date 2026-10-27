//! Слои HTML внутри `<foreignObject>`: содержимое превращается в блочную коробку заданного
//! размера без эффектов SVG-хоста, раскладывается как корень с плотностью по масштабу, PNG
//! снимка кэшируется по хэшу поддерева, стиля и размеров, а узел подменяется на `<image>` с
//! `data:`-ссылкой на снимок.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::box_input::{BoxOrigin, LayoutBoxInput};
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{BoxStyle, Display, Overflow, Position};
use crate::style::values::value::Len;
use gpui::{AnyElement, App, AvailableSpace, Pixels, Size, Window, px};
use std::hash::{Hash, Hasher};

pub(super) struct Layer {
    pub node_id: u64,
    pub key: u64,
    pub size: Size<Pixels>,
    pub density: f32,
    pub element: Option<AnyElement>,
    pub cached: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    e: &mut Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    viewport: (f32, f32),
    scale: f32,
    window: &mut Window,
    cx: &mut App,
    layers: &mut Vec<Layer>,
) {
    let vcx = opts.cx.vcx();
    let merged = box_style(inherited, &e.style, vcx);
    let scale = scale * super::geometry::scale(e);
    if e.tag.eq_ignore_ascii_case("foreignObject") {
        let zoom = merged.zoom_eff.unwrap_or(1.0);
        let w =
            super::geometry::length(e.style.used().width, e.attr("width"), viewport.0, zoom, vcx);
        let h = super::geometry::length(
            e.style.used().height,
            e.attr("height"),
            viewport.1,
            zoom,
            vcx,
        );
        if w <= 0.0 || h <= 0.0 {
            e.children.clear();
            return;
        }
        // HTML раскладывается в CSS-пикселях с действующим зумом. SVG 2
        // задаёт содержащий блок в пользовательских единицах; при размещении
        // снимка переход обратно снимает зум ровно один раз.
        let (origin, paint_size) = super::geometry::capture(&merged, (w, h), viewport, zoom);
        let density = (super::super::DENSITY * scale / zoom).clamp(1.0, 8.0);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{e:?}{merged:?}").hash(&mut hasher);
        // Raster identity includes document content and the application text
        // defaults; it does not depend on which document owns identical HTML.
        format!("{:?}", opts.text).hash(&mut hasher);
        opts.normal_line_height.to_bits().hash(&mut hasher);
        (opts.viewport.0.to_bits(), opts.viewport.1.to_bits()).hash(&mut hasher);
        density.to_bits().hash(&mut hasher);
        (f32::from(origin.x).to_bits(), f32::from(origin.y).to_bits()).hash(&mut hasher);
        (w.to_bits(), h.to_bits(), window.scale_factor().to_bits()).hash(&mut hasher);
        let key = hasher.finish();
        let cached = vcx.doc.foreign_rasters.get(key);
        let element = if cached.is_none() {
            let mut html = e.clone();
            html.tag = "div".into();
            html.attrs.clear();
            html.marks = Default::default();
            html.set_layout_box(LayoutBoxInput::new(
                BoxOrigin::EmbeddedRoot,
                embedded_root(merged.without_paint_filter(), (w * zoom, h * zoom)),
            ));
            let root = BoxStyle::default();
            let mut element = crate::interactive::scroll_box::scrollable(&html, &root, opts)
                .unwrap_or_else(|| opts.cx.contracts.build.element(&html, &root, opts));
            let size = gpui::size(
                AvailableSpace::Definite(px(w * zoom)),
                AvailableSpace::Definite(px(h * zoom)),
            );
            window.with_scene_capture_scale(density, |window| {
                window.with_absolute_transformation(gpui::TransformationMatrix::unit(), |window| {
                    window.with_content_mask_replaced(
                        gpui::ContentMask {
                            bounds: gpui::Bounds {
                                origin: gpui::Point::default(),
                                size: paint_size,
                            },
                        },
                        |window| {
                            window.with_mask_scale(gpui::Point::default(), 1.0, |window| {
                                element.prepaint_as_root(origin, size, window, cx);
                            });
                        },
                    );
                });
            });
            Some(element)
        } else {
            None
        };
        layers.push(Layer {
            node_id: e.node_id,
            key,
            size: paint_size,
            density,
            element,
            cached,
        });
        e.tag = "image".into();
        e.children.clear();
        e.attrs.retain(|(name, _)| {
            !matches!(name.as_str(), "href" | "xlink:href" | "preserveAspectRatio")
        });
        for (name, value) in [("width", w), ("height", h)] {
            e.attrs.retain(|(key, _)| key != name);
            e.attrs.push((name.into(), value.to_string()));
        }
        e.attrs.push(("preserveAspectRatio".into(), "none".into()));
        if origin != gpui::Point::default() {
            super::geometry::expand(e, origin, paint_size, zoom);
        }
        return;
    }
    let viewport = if e.tag.eq_ignore_ascii_case("svg") {
        crate::dom::resource_facts::view_box_ratio(e).unwrap_or(viewport)
    } else {
        viewport
    };
    for child in &mut e.children {
        if let Node::Element(child) = child {
            prepare(child, &merged, opts, viewport, scale, window, cx, layers);
        }
    }
}

/// The box of `foreignObject` as the root of its embedded HTML viewport:
/// a block of the viewport size whose SVG effects (transforms, opacity,
/// clipping, masking, filters, blending) the SVG host already applies to the
/// captured layer; its contents are clipped to the viewport.
fn embedded_root(mut root: BoxStyle, size: (f32, f32)) -> BoxStyle {
    root.width = Some(Len::Px(size.0));
    root.height = Some(Len::Px(size.1));
    root.display = Some(Display::Block);
    root.position = Some(Position::Relative);
    root.flow_root = Some(true);
    root.transform = None;
    root.transform_raw = None;
    root.translate = None;
    root.opacity = None;
    root.clip_ref = None;
    root.clip_inset = None;
    root.clip_edges = None;
    root.clip_xywh = None;
    root.clip_shape = None;
    root.clip_polygon = None;
    root.clip_bare_box = false;
    root.mask_image = None;
    root.filter = None;
    root.filter_ref = None;
    root.rotate_prop = None;
    root.scale_prop = None;
    root.overflow_x.get_or_insert(Overflow::Hidden);
    root.overflow_y.get_or_insert(Overflow::Hidden);
    root.blend = None;
    root
}

pub(super) fn cache(key: u64, png: &[u8], vcx: ValueCx<'_>) -> String {
    use std::fmt::Write as _;
    let mut uri = String::from("data:image/png,");
    for byte in png {
        let _ = write!(uri, "%{byte:02X}");
    }
    vcx.doc.foreign_rasters.remember(key, uri.clone());
    uri
}

pub(super) fn set_image(e: &mut Element, id: u64, uri: String) {
    if e.node_id == id && e.tag == "image" {
        e.attrs.push(("href".into(), uri));
        return;
    }
    for child in &mut e.children {
        if let Node::Element(child) = child {
            set_image(child, id, uri.clone());
        }
    }
}

#[cfg(test)]
mod tests;
