//! SVG serialization uses explicit child cursors with scoped viewport state.

use super::*;

pub(in super::super) fn write_with(
    root: &Element,
    out: &mut String,
    ids: &super::super::ClipIds,
    cx: SerializeCx,
    vcx: ValueCx<'_>,
) {
    let Some(cx) = write_open(root, out, ids, cx, vcx) else {
        return;
    };
    let mut stack = vec![(root, root.children.iter(), cx)];
    while let Some((element, children, cx)) = stack.last_mut() {
        match children.next() {
            Some(Node::Text(text)) => escape_text(text, out),
            Some(Node::Element(child)) => {
                if let Some(child_cx) = write_open(child, out, ids, *cx, vcx) {
                    stack.push((child, child.children.iter(), child_cx));
                }
            }
            None => {
                out.push_str("</");
                out.push_str(&element.tag);
                out.push('>');
                stack.pop();
            }
        }
    }
}

fn write_open(
    e: &Element,
    out: &mut String,
    ids: &super::super::ClipIds,
    cx: SerializeCx,
    vcx: ValueCx<'_>,
) -> Option<SerializeCx> {
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
    if e.children.is_empty() {
        out.push_str("/>");
        return None;
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
    Some(child_cx)
}
