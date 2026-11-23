//! Matching state read through an ancestor, detached from revision-local caches.
use crate::style::select::Ancestor;

#[derive(Clone, Default, PartialEq)]
pub struct SelectorContext {
    pub html_attrs: bool,
    pub visited: bool,
    pub dir: Option<bool>,
    pub has_marks: Vec<u64>,
    pub shadow_has_marks: Vec<u64>,
    pub target_current: bool,
    pub checked: bool,
    pub empty: bool,
    pub foreign: bool,
    pub position: [usize; 4],
}

impl SelectorContext {
    pub(crate) fn capture(node: &Ancestor) -> Self {
        Self {
            html_attrs: node.html_attrs,
            visited: node.visited,
            dir: node.dir,
            has_marks: node.has_marks.clone(),
            shadow_has_marks: node.shadow_has_marks.clone(),
            target_current: node.target_current,
            checked: node.checked,
            empty: node.empty,
            foreign: node.foreign,
            position: [
                node.spot.index,
                node.spot.total,
                node.spot.of_type,
                node.spot.of_type_total,
            ],
        }
    }
}
