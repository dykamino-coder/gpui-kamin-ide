//! Исходная OOF-геометрия группы в реальной page area, без paint-копий.

use super::paint::Paint;
use crate::dom::Node;
use crate::layout::positioned::containing_block::metrics::layout::{self, PageSource};
use std::rc::Rc;

impl Paint<'_> {
    pub(super) fn oof_source(&self) -> Option<PageSource> {
        if !self
            .group
            .iter()
            .any(|n| matches!(n, Node::Element(e) if layout::contains_positioned(e)))
        {
            return None;
        }
        let nodes = self.group.to_vec();
        let root = self.root.clone();
        let opts = Box::new(self.opts.clone());
        let padding = self.padding;
        let first = self.first;
        let vertical = self.vertical;
        Some(Rc::new(move |area, window, cx| {
            let frame = super::frame::wrap(padding, area.1, first, vertical);
            layout::collect_paged(&nodes, &root, (area, frame), &opts, window, cx)
        }))
    }
}
