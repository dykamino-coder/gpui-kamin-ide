//! Независимые дети пробной раскладки и геометрия исходных OOF-потоков.

use super::Recipe;
use crate::layout::fragment::types::StackChild;
use crate::layout::multicol::container::kids::{column_kids, stack_children};
use crate::layout::multicol::measure::laid;
use crate::layout::positioned::containing_block::metrics::Snapshot;
use gpui::{App, Window};
use std::cell::RefCell;

impl Recipe {
    #[cfg(test)]
    pub(in crate::layout::multicol) fn children(
        &self,
        width: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<StackChild> {
        self.recorded_children(width, window, cx).0
    }

    pub(super) fn recorded_children(
        &self,
        width: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> (Vec<StackChild>, std::rc::Rc<Snapshot>) {
        let measured = RefCell::default();
        let vertical = self.axis.is_vertical();
        let reverse = self.axis == crate::layout::fragment::types::StackAxis::VerticalRl;
        let host = self.resolved_host(width);
        let opts = self.geometry_options(&host, width, window, cx);
        let geometry = opts.cx.position.geometry.clone().unwrap_or_default();
        let vcx = opts.cx.vcx();
        let kids = if vertical {
            laid::vertical_children(&host, &self.merged, vcx, width, reverse, &measured)
        } else {
            laid::children(&host, &self.merged, vcx, width, &measured).or_else(|| {
                crate::layout::multicol::measure::native_float::children(
                    &host,
                    &self.merged,
                    vcx,
                    width,
                    &measured,
                )
            })
        };
        let kids = kids.expect("deferred recipe preserves the measured-tree contract");
        let copies = self.count.max(1) as usize;
        let (parents, _, parallel, kids) = column_kids(
            &self.merged,
            kids,
            self.count,
            self.gap,
            vertical,
            self.rows,
            copies,
            self.fixed,
            (Some(width), Some(width)),
            &opts,
        );
        let prepared = kids
            .iter()
            .enumerate()
            .map(|(ix, (c, s))| {
                crate::layout::multicol::stack_child::prepare(
                    ix,
                    c,
                    s,
                    parents[ix].as_ref().unwrap_or(&self.merged),
                    &opts,
                    &parallel,
                    vertical,
                    reverse,
                    Some(width),
                    self.fixed,
                    &measured,
                )
            })
            .collect();
        let children = stack_children(
            &host,
            &self.merged,
            &opts,
            self.count,
            vertical,
            reverse,
            (Some(width), Some(width)),
            self.rows,
            RefCell::default(),
            RefCell::default(),
            measured,
            copies,
            self.fixed,
            parents,
            parallel,
            kids,
            Vec::new(),
            prepared,
        );
        (children, geometry)
    }
}
