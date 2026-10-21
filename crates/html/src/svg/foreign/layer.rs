//! Слои HTML внутри `<foreignObject>`: содержимое превращается в блочную коробку заданного
//! размера без эффектов SVG-хоста, раскладывается как корень с плотностью по масштабу, PNG
//! снимка кэшируется по хэшу поддерева, стиля и размеров, а узел подменяется на `<image>` с
//! `data:`-ссылкой на снимок.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display, Overflow, Position};
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
    inherited: &Computed,
    opts: &RenderOpts,
    viewport: (f32, f32),
    scale: f32,
    window: &mut Window,
    cx: &mut App,
    layers: &mut Vec<Layer>,
) {
    let vcx = opts.cx.vcx();
    let merged = inherit(inherited, &e.style, vcx);
    let scale = scale * super::geometry::scale(e);
    if e.tag.eq_ignore_ascii_case("foreignObject") {
        let w = length(e.style.width, e.attr("width"), viewport.0, vcx);
        let h = length(e.style.height, e.attr("height"), viewport.1, vcx);
        if w <= 0.0 || h <= 0.0 {
            e.children.clear();
            return;
        }
        let (origin, paint_size) = super::geometry::capture(&merged, (w, h), viewport);
        let density = (super::super::DENSITY * scale).clamp(1.0, 8.0);
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
        let cached = vcx.doc.foreign_rasters.borrow().get(&key).cloned();
        let element = if cached.is_none() {
            let mut html = e.clone();
            html.tag = "div".into();
            html.attrs.clear();
            html.marks = Default::default();
            html.style = merged.into();
            html.style.width = Some(Len::Px(w));
            html.style.height = Some(Len::Px(h));
            html.style.display = Some(Display::Block);
            html.style.position = Some(Position::Relative);
            html.style.flow_root = Some(true);
            html.style.transform = None;
            html.style.transform_raw = None;
            html.style.translate = None;
            html.style.opacity = None;
            html.style.clip_ref = None;
            html.style.clip_inset = None;
            html.style.clip_edges = None;
            html.style.clip_xywh = None;
            html.style.clip_shape = None;
            html.style.clip_polygon = None;
            html.style.clip_bare_box = false;
            html.style.mask_image = None;
            html.style.filter = None;
            html.style.filter_ref = None;
            html.style.rotate_prop = None;
            html.style.scale_prop = None;
            html.style.overflow_x.get_or_insert(Overflow::Hidden);
            html.style.overflow_y.get_or_insert(Overflow::Hidden);
            html.style.blend = None;
            let root = Computed::default();
            let mut element = crate::interactive::scroll_box::scrollable(&html, &root, opts)
                .unwrap_or_else(|| opts.cx.build.element(&html, &root, opts));
            let size = gpui::size(
                AvailableSpace::Definite(px(w)),
                AvailableSpace::Definite(px(h)),
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
            super::geometry::expand(e, origin, paint_size);
        }
        return;
    }
    let viewport = if e.tag.eq_ignore_ascii_case("svg") {
        super::super::size::view_box_ratio(e).unwrap_or(viewport)
    } else {
        viewport
    };
    for child in &mut e.children {
        if let Node::Element(child) = child {
            prepare(child, &merged, opts, viewport, scale, window, cx, layers);
        }
    }
}

fn length(style: Option<Len>, attr: Option<&str>, base: f32, vcx: ValueCx<'_>) -> f32 {
    let value = style.or_else(|| attr.and_then(|s| Len::parse(s, vcx)));
    match value {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(v)) => base * v,
        _ => 0.0,
    }
}

pub(super) fn cache(key: u64, png: &[u8], vcx: ValueCx<'_>) -> String {
    use std::fmt::Write as _;
    let mut uri = String::from("data:image/png,");
    for byte in png {
        let _ = write!(uri, "%{byte:02X}");
    }
    {
        let mut cache = vcx.doc.foreign_rasters.borrow_mut();
        if cache.len() >= 64 {
            cache.clear();
        }
        cache.insert(key, uri.clone());
    }
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
