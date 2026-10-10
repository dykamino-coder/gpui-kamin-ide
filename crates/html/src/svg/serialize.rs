//! Обратная сериализация поддерева `<svg>` в разметку для растеризатора.

mod transform;
pub(super) use transform::element_transform;

mod attributes;
pub(super) use attributes::write_attributes;

mod sizing;
pub(super) use sizing::serialize_sized;

mod filters;
use filters::subtree_has;
use filters::untaint_filters;

use super::clip::synth_clip;
use super::size::{size_of, view_box_ratio};
use super::{IN_CLIP, VIEW_BOX};
use crate::dom::{Element, Node};

/// Обратная сериализация поддерева в разметку SVG.
///
/// Дерево у нас уже разобрано, исходного текста нет — а растеризатору нужен
/// именно текст. Пишем только то, что имеет смысл внутри SVG: имя тега,
/// атрибуты и текстовые узлы.
pub fn serialize(e: &Element) -> String {
    let mut out = String::new();
    if subtree_has(e, "fedisplacementmap") {
        let mut copy = e.clone();
        untaint_filters(&mut copy);
        write_element(&copy, &mut out);
    } else {
        write_element(e, &mut out);
    }
    out
}

pub(crate) fn write_element(e: &Element, out: &mut String) {
    // Синтезированный `<clipPath>` пишется ПЕРЕД элементом (см. `synth_clip`).
    let clip_id = synth_clip(e, out);
    out.push('<');
    out.push_str(&e.tag);
    // `transform-origin` растеризатор не знает — точка отсчёта
    // вкатывается в сам transform парой translate. Одно значение — x,
    // второй осью служит середина fill-box (css-transforms §4);
    // доли — от fill-box (атрибуты width/height фигуры).
    let (combined, attr_bad) = element_transform(e);
    write_attributes(e, out, &combined, &clip_id, attr_bad);
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
    if e.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    // Вложенный `<svg>` — свой вьюпорт для `view-box` детей; дети
    // `<clipPath>` — без синтеза своей обрезки (см. `IN_CLIP`).
    let prev_vb = VIEW_BOX.with(|v| v.get());
    if e.tag.eq_ignore_ascii_case("svg") {
        let vb = view_box_ratio(e).unwrap_or_else(|| size_of(e));
        VIEW_BOX.with(|v| v.set(vb));
    }
    let prev_clip = IN_CLIP.with(|c| c.get());
    if e.tag.eq_ignore_ascii_case("clippath") {
        IN_CLIP.with(|c| c.set(true));
    }
    for child in &e.children {
        match child {
            Node::Text(t) => escape_text(t, out),
            Node::Element(el) => write_element(el, out),
        }
    }
    IN_CLIP.with(|c| c.set(prev_clip));
    VIEW_BOX.with(|v| v.set(prev_vb));
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

#[allow(clippy::too_many_arguments)]
pub(super) fn write_paint(e: &Element, out: &mut String) {
    let has = |name: &str| e.attrs.iter().any(|(k, _)| k == name);
    if e.tag != "svg" {
        if let Some(crate::style::values::value::Len::Px(w)) = e.style.width
            && !has("width")
        {
            out.push_str(&format!(" width=\"{w}\""));
        }
        if let Some(crate::style::values::value::Len::Px(h)) = e.style.height
            && !has("height")
        {
            out.push_str(&format!(" height=\"{h}\""));
        }
    }
    if let Some(fill) = &e.style.svg_fill
        && !has("fill")
    {
        out.push_str(" fill=\"");
        escape_attr(fill, out);
        out.push('"');
    }
    // Обводка и геометрия из КАСКАДА: правило `rect.frame { x: -0.5px;
    // stroke: black }` живёт в `<style>` с селектором, в разметке фигуры его
    // нет, а растеризатор видит только разметку. Разметка сильнее: свой
    // атрибут не перебиваем — как у `fill` выше.
    if let Some(stroke) = &e.style.svg_stroke
        && !has("stroke")
    {
        out.push_str(" stroke=\"");
        escape_attr(stroke, out);
        out.push('"');
    }
    if let Some(w) = &e.style.svg_stroke_width
        && !has("stroke-width")
    {
        // `vector-effect: non-scaling-stroke` из КАСКАДА (SVG 2
        // §vector-effect): толщина — в точках экрана. Само свойство в разметку
        // не уходит, поэтому собственный масштаб фигуры снимается с толщины
        // здесь: равномерный множитель стилевой матрицы √|det| (масштаб
        // предков не учитывается). Без этого `svgbox-stroke-box-003/004` при
        // верной геометрии рисовали обводку вдвое тоньше эталонной.
        let k = match (e.style.svg_non_scaling, e.style.transform) {
            (Some(true), Some(t)) if !has("vector-effect") => (t.lin[0][0] * t.lin[1][1]
                - t.lin[0][1] * t.lin[1][0])
                .abs()
                .sqrt(),
            _ => 1.0,
        };
        match w.trim().trim_end_matches("px").parse::<f32>() {
            Ok(v) if k > 1e-6 && (k - 1.0).abs() > 1e-6 => {
                out.push_str(&format!(" stroke-width=\"{}\"", v / k));
            }
            _ => {
                out.push_str(" stroke-width=\"");
                escape_attr(w, out);
                out.push('"');
            }
        }
    }
    if e.tag != "svg" {
        if let Some(crate::style::values::value::Len::Px(x)) = e.style.svg_x
            && !has("x")
        {
            out.push_str(&format!(" x=\"{x}\""));
        }
        if let Some(crate::style::values::value::Len::Px(y)) = e.style.svg_y
            && !has("y")
        {
            out.push_str(&format!(" y=\"{y}\""));
        }
    }
}

pub(super) fn svg_box_style(c: &crate::style::computed::Computed) -> (Option<u8>, Option<bool>) {
    (c.transform_box, c.transform_box_fill)
}
