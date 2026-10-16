//! Shared marker resolution for block and inline list items.
use super::tabular_marker;
use crate::dom::{Element, Node};
use crate::layout::table::anon::anon_element;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};

pub(crate) struct Marker {
    pub text: String,
    pub style: Option<Computed>,
}

pub(crate) fn resolve(li: &Element, inherited: &Computed, merged: &Computed) -> Option<Marker> {
    let idx = li.list_item.unwrap_or(0);
    // Вид маркера задаёт документ; без указания — умолчание тега.
    // Строковый маркер берётся дословно и без суффикса-точки.
    let text_marker = li.style.marker_text.clone().or_else(|| {
        (li.style.list_style_type.is_none()
            && li.style.no_marker.is_none()
            && inherited
                .marker_layer
                .as_deref()
                .is_none_or(|m| m.content.is_none()))
        .then(|| inherited.marker_text.clone())
        .flatten()
    });
    let kind = li
        .style
        .list_style_type
        .clone()
        .or_else(|| inherited.list_style_type.clone());
    let marker = if let Some(t) = text_marker {
        t
    } else {
        // Умолчание тега: у нумерованного перечня десятичный счёт, у
        // списка возможностей — точка.
        let name = kind.unwrap_or_else(|| "disc".to_string());
        crate::style::generated::counter_style::marker_repr(
            idx,
            &name,
            crate::style::generated::counter_style::Context::new(merged, Default::default()),
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
    let no_marker = li.style.no_marker.or(inherited.no_marker) == Some(true)
        || !matches!(li.style.display, None | Some(Display::ListItem));
    let style = li.style.marker_layer.as_deref().map(|m| inherit(merged, m));
    (!no_marker).then_some(Marker {
        text: marker,
        style,
    })
}

pub(crate) fn inline_marker(li: &Element, inherited: &Computed, merged: &Computed) -> Option<Node> {
    Some(node(li, merged, resolve(li, inherited, merged)?))
}

pub(crate) fn node(li: &Element, merged: &Computed, marker: Marker) -> Node {
    let mut span = anon_element("::marker", vec![Node::Text(marker.text)]);
    span.inline = true;
    span.style = marker.style.unwrap_or_default();
    tabular_marker(&mut span.style, merged, li.style.marker_layer.as_deref());
    span.style.content = None;
    span.style.marker_layer = None;
    // css-lists-3 §marker-properties UA rule `::marker { unicode-bidi:
    // isolate }`: Arabic-Indic digits and the suffix stay one run
    // (`css3-counter-styles-101`, reference `<bdi>١. </bdi>`).
    if span.style.bidi_isolate.is_none()
        && span.style.bidi_embed.is_none()
        && span.style.bidi_override.is_none()
        && span.style.bidi_plaintext.is_none()
    {
        span.style.bidi_isolate = Some(true);
    }
    // CSS Pseudo 4 #first-letter-application excludes marker content.
    span.style.first_letter_excluded = true;
    Node::Element(span)
}
