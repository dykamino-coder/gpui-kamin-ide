//! Обратная сериализация поддерева `<svg>` в разметку для растеризатора.

use crate::defaults::DEFAULT_REPLACED_SIZE_PX;
use crate::values::ValueCx;
mod transform;
pub(super) use transform::element_transform;

mod attributes;
pub(super) use attributes::write_attributes;

mod sizing;
pub use sizing::{serialize_sized, serialize_viewport};

mod filters;

mod geometry;
mod paint;
use paint::write_paint;
mod color;
use filters::subtree_has;
use filters::untaint_filters;

use super::clip::synth_clip;
use crate::dom::resource_facts::view_box_ratio;
use crate::resources::svg_document::size::size_of;
/// Viewport and clip content model of this serialization traversal.
#[derive(Clone, Copy)]
pub(super) struct SerializeCx {
    pub(super) view_box: (f32, f32),
    pub(super) in_clip: bool,
    pub(super) color: crate::style::values::value::Color,
}

impl Default for SerializeCx {
    fn default() -> Self {
        Self {
            view_box: DEFAULT_REPLACED_SIZE_PX,
            in_clip: false,
            color: crate::style::values::value::Color::BLACK,
        }
    }
}
use crate::dom::{Element, Node};

/// Обратная сериализация поддерева в разметку SVG.
///
/// Дерево у нас уже разобрано, исходного текста нет — а растеризатору нужен
/// именно текст. Пишем только то, что имеет смысл внутри SVG: имя тега,
/// атрибуты и текстовые узлы.
pub fn serialize(e: &Element, vcx: ValueCx<'_>) -> String {
    crate::work::add!(SvgSerializeCalls, 1);
    crate::work::time!(_svg_time, SvgSerializeNs);
    let mut out = String::new();
    if subtree_has(e, "fedisplacementmap") {
        let mut copy = e.clone();
        untaint_filters(&mut copy);
        write_element(&copy, &mut out, vcx);
    } else {
        write_element(e, &mut out, vcx);
    }
    out
}

/// Writes `e` as its own document of synthesized ids.
pub fn write_element(e: &Element, out: &mut String, vcx: ValueCx<'_>) {
    write_in(e, out, &super::ClipIds::for_document(e), vcx);
}

/// Writes `e` inside a document whose synthesized ids come from `ids`.
pub fn write_in(e: &Element, out: &mut String, ids: &super::ClipIds, vcx: ValueCx<'_>) {
    write_with(e, out, ids, SerializeCx::default(), vcx);
}

pub(super) fn write_with(
    e: &Element,
    out: &mut String,
    ids: &super::ClipIds,
    cx: SerializeCx,
    vcx: ValueCx<'_>,
) {
    crate::work::add!(SvgSerializeNodes, 1);
    // Синтезированный `<clipPath>` пишется ПЕРЕД элементом (см. `synth_clip`).
    let clip_id = synth_clip(e, out, ids, cx, vcx);
    out.push('<');
    out.push_str(&e.tag);
    // `transform-origin` растеризатор не знает — точка отсчёта
    // вкатывается в сам transform парой translate. Одно значение — x,
    // второй осью служит середина fill-box (css-transforms §4);
    // доли — от fill-box (атрибуты width/height фигуры).
    let (combined, attr_bad) = element_transform(e, cx.view_box);
    write_attributes(e, out, &combined, &clip_id, attr_bad);
    geometry::write_geometry(e, out);
    if let Some(t) = &combined {
        out.push_str(" transform=\"");
        escape_attr(t, out);
        out.push('"');
    }
    if let Some(id) = &clip_id {
        out.push_str(&format!(" clip-path=\"url(#{id})\""));
    }
    // CSS-геометрия и заливка SVG-фигур (SVG 2): стилевые ширина/высота
    // и `fill` доезжают до растеризатора презентационными атрибутами,
    // если разметка своих не задала.
    write_paint(e, out);
    let color = color::write(e, cx.color, out);
    if e.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    // Вложенный `<svg>` — свой вьюпорт для `view-box` детей; дети
    // `<clipPath>` — без синтеза своей обрезки.
    let mut child_cx = cx;
    child_cx.color = color;
    if e.tag.eq_ignore_ascii_case("svg") {
        let vb = view_box_ratio(e).unwrap_or_else(|| size_of(e));
        child_cx.view_box = vb;
    }
    if e.tag.eq_ignore_ascii_case("clippath") {
        child_cx.in_clip = true;
    }
    for child in &e.children {
        match child {
            Node::Text(t) if e.tag == "style" => escape_text(&color::stylesheet(t), out),
            Node::Text(t) => escape_text(t, out),
            Node::Element(el) => write_with(el, out, ids, child_cx, vcx),
        }
    }
    out.push_str("</");
    out.push_str(&e.tag);
    out.push('>');
}

pub(super) fn escape_attr(v: &str, out: &mut String) {
    for ch in v.chars() {
        match ch {
            '"' => out.push_str("&quot;"),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            c => out.push(c),
        }
    }
}

fn escape_text(v: &str, out: &mut String) {
    for ch in v.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
}

pub(super) fn svg_box_style(c: &crate::style::computed::Computed) -> (Option<u8>, Option<bool>) {
    (
        c.reset_transforms.transform_box,
        c.reset_transforms.transform_box_fill,
    )
}

#[cfg(test)]
mod context_tests;

#[cfg(test)]
mod color_tests;
