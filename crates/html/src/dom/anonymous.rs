//! Creation of anonymous boxes without authored node identity or pseudo styles.

use super::{Element, Marks, Node};
use crate::style::computed::Computed;

impl Element {
    /// A layout box has explicit geometry and marks, but no source attributes.
    pub(crate) fn anonymous(
        tag: &str,
        style: Computed,
        children: Vec<Node>,
        inline: bool,
        marks: Marks,
    ) -> Self {
        Self {
            tag: tag.into(),
            style: style.into(),
            children,
            inline,
            marks,
            list_item: None,
            node_id: 0,
            attrs: Vec::new(),
            anim: None,
            hover: None,
            first_letter: None,
            first_line: None,
        }
    }
}
