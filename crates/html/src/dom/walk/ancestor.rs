//! Паспорт элемента (`Ancestor`) для сопоставления селекторов — отдельно от
//! рекурсии обхода.
//!
//! Вход — узел html5ever, тег, атрибуты, место среди братьев; выход —
//! `Ancestor` с `id`, классами, `href`, `dir`, отметками `:has()`, слотом,
//! братьями, признаками пустоты и чужого пространства имён. Счётчики и
//! `content` в паспорт пишет позже `content::snapshot_generated`.

use crate::dom::tree_index::TreeIndex;
use crate::style::select::{Ancestor, Sibs, Spot};
use markup5ever_rcdom::Handle;

#[allow(clippy::ptr_arg, clippy::let_and_return)]
pub(super) fn element_ancestor(
    handle: &Handle,
    tag: &str,
    attrs: &Vec<(String, String)>,
    html_attrs: bool,
    spot: Spot,
    sibs: Sibs,
    tree: &TreeIndex,
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
        counter_style: Default::default(),
        counter_context: Default::default(),
        content_style: None,
        content_none: None,
        tag: tag.to_string(),
        html_attrs,
        id,
        classes,
        attrs: attrs.clone(),
        spot,
        href: attrs
            .iter()
            .find(|(k, _)| k == "href")
            .map(|(_, v)| v.clone()),
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
    };

    me
}

/// The element's attributes as `(name, value)`; `xml:base` keeps its prefix
/// (the only namespaced attribute the engine reads).
pub(super) fn element_attrs(attrs: &[html5ever::Attribute]) -> Vec<(String, String)> {
    attrs
        .iter()
        .map(|a| {
            let key = if &*a.name.local == "base"
                && &*a.name.ns == "http://www.w3.org/XML/1998/namespace"
            {
                "xml:base".to_owned()
            } else {
                a.name.local.to_string()
            };
            (key, a.value.to_string())
        })
        .collect()
}
