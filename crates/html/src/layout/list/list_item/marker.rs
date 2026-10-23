//! Общее разрешение маркера для блочных и строчных пунктов списка: текст маркера
//! (`content` у `::marker`, `list-style-type`, счётчик пункта), признак `list-style: none`
//! и чужого `display`, стиль слоя `::marker` и сборка анонимного узла `::marker`
//! с UA-правилами css-lists-3 (`unicode-bidi: isolate`, без `letter-spacing` у символа).

use super::tabular_marker;
use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::table::anon::anon_element;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};

pub(crate) struct Marker {
    pub text: String,
    pub style: Option<crate::style::computed::BoxStyle>,
}

pub(crate) fn resolve(
    li: &Element,
    inherited: &Computed,
    merged: &crate::style::computed::BoxStyle,
    vcx: ValueCx<'_>,
) -> Option<Marker> {
    let idx = li.list_item.unwrap_or(0);
    // Вид маркера задаёт документ; без указания — умолчание тега.
    // Строковый маркер берётся дословно и без суффикса-точки.
    let text_marker = li.style.computed().marker_text.clone().or_else(|| {
        (li.style.cascaded().list_style_type.is_none()
            && li.style.computed().no_marker.is_none()
            && inherited
                .marker_layer
                .as_deref()
                .is_none_or(|m| m.content.is_none()))
        .then(|| inherited.marker_text.clone())
        .flatten()
    });
    let kind = li
        .style
        .cascaded()
        .list_style_type
        .clone()
        .or_else(|| inherited.list_style_type.clone());
    let symbol = text_marker_symbol(
        text_marker.is_none()
            && li
                .style
                .computed()
                .marker_layer
                .as_deref()
                .is_none_or(|m| m.content.is_none()),
        kind.as_deref(),
    );
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
    let no_marker = li.style.computed().no_marker.or(inherited.no_marker) == Some(true)
        || !matches!(li.style.computed().display, None | Some(Display::ListItem));
    let mut style = li
        .style
        .computed()
        .marker_layer
        .as_deref()
        .map(|m| inherit(merged, m, vcx));
    // CSS Lists 3 marker-properties: an outside generated symbol has no
    // tracking advance (Blink list_marker.cc:391,407, InlineMarginsForOutside).
    // Preserve text markers and the inside-marker text flow from #355.
    if symbol && merged.list_style_inside != Some(true) {
        style.get_or_insert_with(|| merged.clone()).letter_spacing =
            Some(crate::style::values::value::Len::Px(0.0));
    }
    (!no_marker).then_some(Marker {
        text: marker,
        style,
    })
}

pub(crate) fn inline_marker(
    li: &Element,
    inherited: &Computed,
    merged: &crate::style::computed::BoxStyle,
    vcx: ValueCx<'_>,
) -> Option<Node> {
    Some(node(li, merged, resolve(li, inherited, merged, vcx)?))
}

pub(crate) fn node(li: &Element, merged: &Computed, marker: Marker) -> Node {
    let mut span = anon_element("::marker", vec![Node::Text(marker.text)]);
    span.inline = true;
    span.style = marker.style.unwrap_or_default().into();
    tabular_marker(
        span.style.used_mut(),
        merged,
        li.style.computed().marker_layer.as_deref(),
    );
    span.style.used_mut().content = None;
    span.style.used_mut().marker_layer = None;
    // css-lists-3 §marker-properties UA rule `::marker { unicode-bidi:
    // isolate }`: Arabic-Indic digits and the suffix stay one run
    // (`css3-counter-styles-101`, reference `<bdi>١. </bdi>`).
    if span.style.computed().bidi_isolate.is_none()
        && span.style.computed().bidi_embed.is_none()
        && span.style.computed().bidi_override.is_none()
        && span.style.computed().bidi_plaintext.is_none()
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
