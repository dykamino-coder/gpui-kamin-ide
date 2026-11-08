//! Паспорт элемента (`Ancestor`) для сопоставления селекторов — отдельно от
//! рекурсии обхода.
//!
//! Вход — узел html5ever, тег, атрибуты, место среди братьев; выход —
//! `Ancestor` с `id`, классами, `href`, `dir`, отметками `:has()`, слотом,
//! братьями, признаками пустоты и чужого пространства имён, языком
//! содержимого (свой `lang` или переданный от родителя по `path`). Счётчики и
//! `content` в паспорт пишет позже `content::snapshot_generated`.

use crate::dom::tree_index::TreeIndex;
use crate::style::select::{Ancestor, Sibs, Spot};
use markup5ever_rcdom::Handle;

/// SVG 2: CSS-коробка есть лишь у внешнего svg, включая svg непосредственно
/// внутри foreignObject. Blink SVGElement::IsOutermostSVGSVGElement;
/// внутренний SVG не поддерживает size containment (css-contain-2 §2).
pub(super) fn svg_no_css_box(handle: &Handle) -> bool {
    use markup5ever_rcdom::NodeData;
    const SVG: &str = "http://www.w3.org/2000/svg";
    let NodeData::Element { name, .. } = &handle.data else {
        return false;
    };
    if &*name.ns != SVG {
        return false;
    }
    if &*name.local != "svg" {
        return true;
    }
    let parent = handle.parent.take();
    let parent_node = parent.as_ref().and_then(|p| p.upgrade());
    handle.parent.set(parent);
    parent_node.is_some_and(|p| {
        matches!(&p.data, NodeData::Element { name, .. }
            if &*name.ns == SVG && &*name.local != "foreignObject")
    })
}

// The passport is built once from the node, its sibling census and the walk path.
#[allow(clippy::too_many_arguments)]
pub(super) fn element_ancestor(
    handle: &Handle,
    tag: &str,
    attrs: &crate::dom::SharedAttrs,
    html_attrs: bool,
    spot: Spot,
    sibs: Sibs,
    tree: &TreeIndex,
    path: &[Ancestor],
) -> Ancestor {
    let id = attrs
        .iter()
        .find(|(k, _)| k == "id")
        .map(|(_, v)| v.clone());
    let classes: Vec<String> = attrs
        .iter()
        .find(|(k, _)| k == "class")
        .map(|(_, v)| v.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default();
    let me = Ancestor {
        key: crate::dom::tree_index::NodeKey::of(handle),
        matching: tree.matching.clone(),
        ancestor_bloom: None,
        counter_style: Default::default(),
        counter_context: Default::default(),
        content_style: None,
        content_none: None,
        tag: tag.into(),
        html_attrs,
        id,
        classes,
        attrs: attrs.clone(),
        spot,
        href: attrs
            .iter()
            .find(|(k, _)| k == "href")
            .map(|(_, v)| v.clone()),
        visited: crate::style::select::link_visited(
            attrs
                .iter()
                .find(|(k, _)| k == "href")
                .map(|(_, v)| v.as_str()),
        ),
        dir: attrs.iter().find(|(k, _)| k == "dir").and_then(|(_, v)| {
            match v.to_ascii_lowercase().as_str() {
                "rtl" => Some(true),
                "ltr" => Some(false),
                _ => None,
            }
        }),
        has_marks: tree.has_marks_of(handle),
        shadow_has_marks: vec![],
        featureless: None,
        slot: tree.slot_of(handle),
        peers: sibs
            .is_elem
            .then_some(sibs.rc)
            .flatten()
            .map(|r| (r.clone(), sibs.pos)),
        target_group: None,
        target_current: false,
        empty: crate::style::select::is_empty_node(handle),
        foreign: match &handle.data {
            markup5ever_rcdom::NodeData::Element { name, .. } => {
                &*name.ns != "http://www.w3.org/1999/xhtml"
            }
            _ => false,
        },
        lang: Some(crate::dom::language::inherited(attrs, path)),
    };

    me
}

/// The element's attribute list, shared with its sibling census entry when it holds the same
/// pairs. Blink keeps an element's attributes once, in its ElementData
/// (third_party/blink/renderer/core/dom/element_data.h:88), and selector matching reads them
/// there; the census (`Sibs::all[pos]`) already read this node's list, so the walk takes it
/// instead of a second copy. Only `xml:base` differs: the census keeps its local name.
pub(super) fn element_attrs_shared(
    handle: &Handle,
    attrs: &[html5ever::Attribute],
    sibs: &Sibs,
) -> crate::dom::SharedAttrs {
    let census = sibs.is_elem.then(|| sibs.all.get(sibs.pos)).flatten();
    match census {
        Some(a)
            if a.key == crate::dom::tree_index::NodeKey::of(handle)
                && !attrs.iter().any(is_xml_base) =>
        {
            a.attrs.clone()
        }
        _ => element_attrs(attrs).into(),
    }
}

/// `xml:base`: the one namespaced attribute whose name keeps its prefix.
fn is_xml_base(a: &html5ever::Attribute) -> bool {
    &*a.name.local == "base" && &*a.name.ns == "http://www.w3.org/XML/1998/namespace"
}

/// The element's attributes as `(name, value)`; `xml:base` keeps its prefix
/// (the only namespaced attribute the engine reads).
pub fn element_attrs(attrs: &[html5ever::Attribute]) -> Vec<(String, String)> {
    attrs
        .iter()
        .map(|a| {
            let key = if is_xml_base(a) {
                "xml:base".to_owned()
            } else {
                a.name.local.to_string()
            };
            (key, a.value.to_string())
        })
        .collect()
}
