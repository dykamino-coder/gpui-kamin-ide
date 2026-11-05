//! Исходная OOF-геометрия группы в реальной page area, без paint-копий.

use super::paint::Paint;
use crate::dom::Node;
use crate::layout::positioned::containing_block::metrics::layout::{self, PageSource};
use std::rc::Rc;

impl Paint<'_> {
    /// The group has OOF boxes; its native source paints them, not the copies.
    pub(super) fn native_oof(&self) -> bool {
        self.group
            .iter()
            .any(|n| matches!(n, Node::Element(e) if layout::contains_oof_box(e)))
    }

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
        let source = Rc::new((nodes, root, opts));
        Some(Rc::new(move |area, window, cx, stack| {
            let source = source.clone();
            Box::pin(async move {
                let (nodes, root, opts) = &*source;
                let frame = || super::frame::wrap(padding, area.1, first, vertical);
                stack
                    .run(|stack| {
                        layout::collect_paged_async(
                            nodes,
                            root,
                            (area, &frame),
                            opts,
                            window,
                            cx,
                            stack,
                        )
                    })
                    .await
            })
        }))
    }
}
