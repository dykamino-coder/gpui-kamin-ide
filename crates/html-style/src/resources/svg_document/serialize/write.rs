//! Сериализация элементов: необязательная типизированная связь с исходным DOM.

use super::*;

pub(super) fn write_traced(
    e: &Element,
    out: &mut String,
    ids: &crate::resources::svg_document::ClipIds,
    cx: SerializeCx,
    vcx: ValueCx<'_>,
    trace: &mut impl FnMut(u64, usize),
) {
    crate::work::add!(SvgSerializeNodes, 1);
    // Синтезированный `<clipPath>` пишется ПЕРЕД элементом (см. `synth_clip`).
    let clip_id = synth_clip(e, out, ids, cx, vcx);
    trace(e.node_id, out.len());
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
    if e.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    // Вложенный `<svg>` — свой вьюпорт для `view-box` детей; дети
    // `<clipPath>` — без синтеза своей обрезки.
    let mut child_cx = cx;
    if e.tag.eq_ignore_ascii_case("svg") {
        let vb = view_box_ratio(e).unwrap_or_else(|| size_of(e));
        child_cx.view_box = vb;
    }
    if e.tag.eq_ignore_ascii_case("clippath") {
        child_cx.in_clip = true;
    }
    for child in &e.children {
        match child {
            Node::Text(t) => escape_text(t, out),
            Node::Element(el) => write_traced(el, out, ids, child_cx, vcx, trace),
        }
    }
    out.push_str("</");
    out.push_str(&e.tag);
    out.push('>');
}
