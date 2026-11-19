//! The source key of a box: the POD of the DOM node apart from the synthetic role.
//!
//! Until Element::node_id moves to M1 the real key comes from the traversal record;
//! the order, density and addresses of boxes are not used as identity.

pub(super) use crate::dom::NodeKey;
use crate::dom::{BoxId, Element};
use std::rc::Rc;
use std::string::String;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum Source {
    Root,
    Viewport,
    Node(NodeKey),
    Synthetic(Rc<Synthetic>),
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub(super) struct Synthetic {
    owner: BoxId,
    tag: String,
    role: u16,
    extent: super::sources::Extent,
}

#[cfg(feature = "heap-report")]
kamin_heap::heap_fields!(Synthetic { tag });

#[cfg(feature = "heap-report")]
impl kamin_heap::HeapSize for Source {
    fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        match self {
            Self::Synthetic(source) => kamin_heap::HeapSize::heap_size(source, ops),
            _ => 0,
        }
    }
}

impl Source {
    pub(super) fn authored(element: &Element) -> bool {
        use crate::style::box_input::BoxOrigin;
        element.node_id != 0
            && !element.marks.anonymous_wrapper
            && !element.marks.anonymous_cell
            && !element.marks.anon
            && !element.marks.spanner_content_box
            && !element.marks.initial_letter
            && !matches!(
                element.marks.box_origin,
                Some(BoxOrigin::Marker | BoxOrigin::EmbeddedRoot)
            )
    }

    pub(super) fn of(
        element: &Element,
        owner: BoxId,
        extent: super::sources::Extent,
        key: Option<NodeKey>,
    ) -> Self {
        if Self::authored(element)
            && let Some(key) = key
        {
            Self::Node(key)
        } else if !owner.is_some() && element.tag == "html-viewport" {
            // The single root box of the window is not a fragment of the
            // first/last child and survives the replacement of their range.
            Self::Viewport
        } else {
            let marks = &element.marks;
            let flags = [
                marks.anonymous_wrapper,
                marks.anonymous_cell,
                marks.anon,
                marks.spanner_content_box,
                marks.initial_letter,
                element.inline,
            ];
            let mut role = flags
                .into_iter()
                .enumerate()
                .fold(0, |role, (bit, set)| role | (u16::from(set) << bit));
            use crate::style::box_input::BoxOrigin;
            role |= match marks.box_origin {
                Some(BoxOrigin::Moved) => 1 << 6,
                Some(BoxOrigin::Marker) => 2 << 6,
                Some(BoxOrigin::EmbeddedRoot) => 3 << 6,
                None => 0,
            };
            Self::Synthetic(Rc::new(Synthetic {
                owner,
                tag: element.tag.clone(),
                role,
                extent,
            }))
        }
    }
}
