//! Общее разрешение маркера для блочных и строчных пунктов списка: текст маркера
//! (`content` у `::marker`, `list-style-type`, счётчик пункта), признак `list-style: none`
//! и чужого `display`, стиль слоя `::marker` и сборка анонимного узла `::marker`
//! с UA-правилами css-lists-3 (`unicode-bidi: isolate`, без `letter-spacing` у символа).

use super::tabular_marker;
use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::table::anon::anon_element;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{BoxStyle, Computed, Display, MarkerType};

mod image;
#[cfg(test)]
mod tests;

pub(crate) struct Marker {
    pub text: String,
    pub style: Option<BoxStyle>,
    pub image: Option<Node>,
}

pub(crate) fn resolve(
    li: &Element,
    _inherited: &Computed,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
) -> Option<Marker> {
    let idx = li.list_item.unwrap_or(0);
    // Вид маркера задаёт документ; без указания — умолчание тега.
    // Строковый маркер берётся дословно и без суффикса-точки.
    let text_marker = merged
        .marker_text
        .clone()
        .or_else(|| match &merged.list_style_type {
            Some(MarkerType::Text(text)) => Some(text.clone()),
            _ => None,
        });
    let kind = match &merged.list_style_type {
        Some(MarkerType::Counter(name)) => Some(name.as_str()),
        _ => None,
    };
    let symbol = text_marker_symbol(
        text_marker.is_none()
            && li
                .style
                .used()
                .marker_layer
                .as_deref()
                .is_none_or(|m| m.content.is_none()),
        kind,
    );
    let marker = if let Some(t) = text_marker {
        t
    } else {
        // Умолчание тега: у нумерованного перечня десятичный счёт, у
        // списка возможностей — точка.
        let name = kind.unwrap_or("disc");
        crate::style::generated::counter_style::marker_repr(
            idx,
            name,
            crate::style::generated::counter_style::Context::new(merged, Default::default()),
            vcx,
        )
    };
    // `list-style: none` — на списках верстают навигацию и наборы чипов,
    // и точки там лишние. Своё слово пункта старше слова списка:
    // `list-style-type` наследуемое, и `<ul style="list-style-type:
    // none">` не гасит `li::marker { content }` (`marker-content-012`).
    //
    // Чужой `display` на пункте снимает с него признак пункта, а с ним и
    // маркер (css-display-3: `list-item` есть только у `display:
    // list-item`; css-pseudo-4 §marker-pseudo: «the computed value of
    // 'display' on ::marker always loses any list-item aspect» — обратное
    // верно тем более). Без этого эталон `marker-content-019-ref`
    // (`li { display: block }`) рисовал у нас полный набор `1. 2. 3. 4.`,
    // и свёртка `content: none` в тесте разводила стороны ЕЩЁ дальше.
    let mut style = li
        .style
        .used()
        .marker_layer
        .as_deref()
        .map(|m| inherit(merged, m, vcx));
    let li_style = li.style.used();
    let layer = li_style.marker_layer.as_deref();
    let normal = layer.is_none_or(|m| m.content.is_none() && m.content_none != Some(true));
    let image = normal
        .then(|| image::resolve(merged, style.as_deref(), vcx))
        .flatten();
    let no_marker = (!matches!(li_style.display, None | Some(Display::ListItem)))
        || merged.no_marker == Some(true)
        || (normal && image.is_none() && merged.list_style_type == Some(MarkerType::None));
    // Внешний геометрический знак не использует advance текстового пробела;
    // строковые и внутренние маркеры остаются частью текстового форматирования.
    if symbol && merged.list_style_inside != Some(true) {
        style = Some(crate::style::cascade::inherit::marker_symbol(
            style, merged, vcx,
        ));
    }
    (!no_marker).then_some(Marker {
        text: marker,
        style,
        image,
    })
}

pub(crate) fn inline_marker(
    li: &Element,
    inherited: &Computed,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
) -> Option<Node> {
    Some(node(li, merged, resolve(li, inherited, merged, vcx)?))
}

pub(crate) fn node(li: &Element, merged: &Computed, marker: Marker) -> Node {
    let children = match marker.image {
        Some(image) => vec![image, Node::Text(" ".into())],
        None => vec![Node::Text(marker.text)],
    };
    let mut span = anon_element("::marker", children);
    span.inline = true;
    span.style = crate::dom::SharedStyle::layout_made(marker.style.unwrap_or_default());
    tabular_marker(
        span.style.used_mut(),
        merged,
        li.style.used().marker_layer.as_deref(),
    );
    span.style.used_mut().content = None;
    span.style.used_mut().marker_layer = None;
    // css-lists-3 §marker-properties UA rule `::marker { unicode-bidi:
    // isolate }`: Arabic-Indic digits and the suffix stay one run
    // (`css3-counter-styles-101`, reference `<bdi>١. </bdi>`).
    if span.style.used().bidi_isolate.is_none()
        && span.style.used().bidi_embed.is_none()
        && span.style.used().bidi_override.is_none()
        && span.style.used().bidi_plaintext.is_none()
    {
        span.style.used_mut().bidi_isolate = Some(true);
    }
    // CSS Pseudo 4 #first-letter-application excludes marker content.
    span.style.used_mut().first_letter_excluded = true;
    Node::Element(span)
}

/// A generated marker drawn as a symbol rather than as text (css-counter-styles
/// predefined `disc`, `circle`, `square`, `disclosure-*`; `disc` is the default).
fn text_marker_symbol(generated: bool, kind: Option<&str>) -> bool {
    generated
        && matches!(
            kind.unwrap_or("disc"),
            "disc" | "circle" | "square" | "disclosure-open" | "disclosure-closed"
        )
}
