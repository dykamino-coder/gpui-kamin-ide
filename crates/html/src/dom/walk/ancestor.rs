//! Element passports keep DOM selector metadata separate from recursive walking.

use crate::dom::slot_of;
use crate::style::select::has::has_marks_of;
use crate::style::select::{Ancestor, Sibs, Spot};
use markup5ever_rcdom::Handle;

#[allow(clippy::ptr_arg, clippy::let_and_return)]
pub(super) fn element_ancestor(
    handle: &Handle,
    tag: &String,
    attrs: &Vec<(String, String)>,
    html_attrs: bool,
    spot: Spot,
    sibs: Sibs,
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
        tag: tag.clone(),
        html_attrs,
        id: id.clone(),
        classes: classes.clone(),
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
        has_marks: has_marks_of(handle),
        featureless: None,
        slot: slot_of(handle),
        peers: sibs
            .is_elem
            .then_some(sibs.rc)
            .flatten()
            .map(|r| (r.clone(), sibs.pos)),
    };

    me
}
