//! РћР±С‰РµРµ СЂР°Р·СЂРµС€РµРЅРёРµ РјР°СЂРєРµСЂР° РґР»СЏ Р±Р»РѕС‡РЅС‹С… Рё СЃС‚СЂРѕС‡РЅС‹С… РїСѓРЅРєС‚РѕРІ СЃРїРёСЃРєР°: С‚РµРєСЃС‚ РјР°СЂРєРµСЂР°
//! (`content` Сѓ `::marker`, `list-style-type`, СЃС‡С‘С‚С‡РёРє РїСѓРЅРєС‚Р°), РїСЂРёР·РЅР°Рє `list-style: none`
//! Рё С‡СѓР¶РѕРіРѕ `display`, СЃС‚РёР»СЊ СЃР»РѕСЏ `::marker` Рё СЃР±РѕСЂРєР° Р°РЅРѕРЅРёРјРЅРѕРіРѕ СѓР·Р»Р° `::marker`
//! СЃ UA-РїСЂР°РІРёР»Р°РјРё css-lists-3 (`unicode-bidi: isolate`, Р±РµР· `letter-spacing` Сѓ СЃРёРјРІРѕР»Р°).

use super::tabular_marker;
use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::table::anon::anon_element;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{BoxStyle, Computed, Display};

mod image;

pub(crate) struct Marker {
    pub text: String,
    pub style: Option<BoxStyle>,
    pub image: Option<Node>,
}

pub(crate) fn resolve(
    li: &Element,
    inherited: &Computed,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
) -> Option<Marker> {
    let idx = li.list_item.unwrap_or(0);
    // Р’РёРґ РјР°СЂРєРµСЂР° Р·Р°РґР°С‘С‚ РґРѕРєСѓРјРµРЅС‚; Р±РµР· СѓРєР°Р·Р°РЅРёСЏ вЂ” СѓРјРѕР»С‡Р°РЅРёРµ С‚РµРіР°.
    // РЎС‚СЂРѕРєРѕРІС‹Р№ РјР°СЂРєРµСЂ Р±РµСЂС‘С‚СЃСЏ РґРѕСЃР»РѕРІРЅРѕ Рё Р±РµР· СЃСѓС„С„РёРєСЃР°-С‚РѕС‡РєРё.
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
        // РЈРјРѕР»С‡Р°РЅРёРµ С‚РµРіР°: Сѓ РЅСѓРјРµСЂРѕРІР°РЅРЅРѕРіРѕ РїРµСЂРµС‡РЅСЏ РґРµСЃСЏС‚РёС‡РЅС‹Р№ СЃС‡С‘С‚, Сѓ
        // СЃРїРёСЃРєР° РІРѕР·РјРѕР¶РЅРѕСЃС‚РµР№ вЂ” С‚РѕС‡РєР°.
        let name = kind.unwrap_or_else(|| "disc".to_string());
        crate::style::generated::counter_style::marker_repr(
            idx,
            &name,
            crate::style::generated::counter_style::Context::new(merged, Default::default()),
            vcx,
        )
    };
    // `list-style: none` вЂ” РЅР° СЃРїРёСЃРєР°С… РІРµСЂСЃС‚Р°СЋС‚ РЅР°РІРёРіР°С†РёСЋ Рё РЅР°Р±РѕСЂС‹ С‡РёРїРѕРІ,
    // Рё С‚РѕС‡РєРё С‚Р°Рј Р»РёС€РЅРёРµ. РЎРІРѕС‘ СЃР»РѕРІРѕ РїСѓРЅРєС‚Р° СЃС‚Р°СЂС€Рµ СЃР»РѕРІР° СЃРїРёСЃРєР°:
    // `list-style-type` РЅР°СЃР»РµРґСѓРµРјРѕРµ, Рё `<ul style="list-style-type:
    // none">` РЅРµ РіР°СЃРёС‚ `li::marker { content }` (`marker-content-012`).
    //
    // Р§СѓР¶РѕР№ `display` РЅР° РїСѓРЅРєС‚Рµ СЃРЅРёРјР°РµС‚ СЃ РЅРµРіРѕ РїСЂРёР·РЅР°Рє РїСѓРЅРєС‚Р°, Р° СЃ РЅРёРј Рё
    // РјР°СЂРєРµСЂ (css-display-3: `list-item` РµСЃС‚СЊ С‚РѕР»СЊРєРѕ Сѓ `display:
    // list-item`; css-pseudo-4 В§marker-pseudo: В«the computed value of
    // 'display' on ::marker always loses any list-item aspectВ» вЂ” РѕР±СЂР°С‚РЅРѕРµ
    // РІРµСЂРЅРѕ С‚РµРј Р±РѕР»РµРµ). Р‘РµР· СЌС‚РѕРіРѕ СЌС‚Р°Р»РѕРЅ `marker-content-019-ref`
    // (`li { display: block }`) СЂРёСЃРѕРІР°Р» Сѓ РЅР°СЃ РїРѕР»РЅС‹Р№ РЅР°Р±РѕСЂ `1. 2. 3. 4.`,
    // Рё СЃРІС‘СЂС‚РєР° `content: none` РІ С‚РµСЃС‚Рµ СЂР°Р·РІРѕРґРёР»Р° СЃС‚РѕСЂРѕРЅС‹ Р•Р©РЃ РґР°Р»СЊС€Рµ.
    let mut style = li
        .style
        .computed()
        .marker_layer
        .as_deref()
        .map(|m| inherit(merged, m, vcx));
    let li_style = li.style.computed();
    let layer = li_style.marker_layer.as_deref();
    let normal = layer.is_none_or(|m| m.content.is_none() && m.content_none != Some(true));
    let image = normal
        .then(|| image::resolve(merged, style.as_deref(), vcx))
        .flatten();
    let no_marker = (!matches!(li_style.display, None | Some(Display::ListItem)))
        || (image.is_none() && li_style.no_marker.or(inherited.no_marker) == Some(true));
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
    span.style = marker.style.unwrap_or_default().into();
    tabular_marker(
        span.style.used_mut(),
        merged,
        li.style.computed().marker_layer.as_deref(),
    );
    span.style.used_mut().content = None;
    span.style.used_mut().marker_layer = None;
    // css-lists-3 В§marker-properties UA rule `::marker { unicode-bidi:
    // isolate }`: Arabic-Indic digits and the suffix stay one run
    // (`css3-counter-styles-101`, reference `<bdi>ЩЎ. </bdi>`).
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
