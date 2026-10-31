//! Независимые дети пробной раскладки и геометрия исходных OOF-потоков.

use super::Recipe;
use crate::layout::fragment::types::StackChild;
use crate::layout::multicol::container::kids::{column_kids, stack_children};
use crate::layout::positioned::containing_block::metrics::Snapshot;
use gpui::{App, Window};

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

    // Сборка детей — свой кадр: раскладка их копий в `prepare` вложена глубже.
    #[inline(never)]
    pub(super) fn recorded_children(
        &self,
        width: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> (Vec<StackChild>, std::rc::Rc<Snapshot>) {
        let vertical = self.axis.is_vertical();
        let reverse = self.axis == crate::layout::fragment::types::StackAxis::VerticalRl;
        let host = self.resolved_host(width);
        let opts = self.geometry_options(&host, width, window, cx);
        let geometry = opts.cx.position.geometry.clone().unwrap_or_default();
        // Тот же dispatch по каждому ребёнку, что у стопки известной ширины.
        let plan = crate::layout::multicol::measure::plan_at(
            &self.host,
            &self.merged,
            &opts,
            (Some(width), Some(width)),
            (vertical, reverse),
            (self.fixed, None),
            self.rows,
            None,
        );
        let measured = plan.measured_kids;
        let kids = plan.stackable.unwrap_or_default();
        let copies = self.count.max(1) as usize;
        let (parents, _, parallel, kids) = column_kids(kids, opts.cx.vcx().clearance_possible);
        let prepared = kids
            .iter()
            .map(|(c, _)| {
                crate::layout::multicol::stack_child::prepare(c, &opts, vertical, reverse)
            })
            .collect();
        let children = stack_children(
            &self.merged,
            &opts,
            self.count,
            vertical,
            reverse,
            (Some(width), Some(width)),
            self.rows,
            plan.nested_whole,
            measured,
            copies,
            self.fixed,
            parents,
            parallel,
            kids,
            prepared,
        );
        let (statics, spots) = &*self.statics.borrow();
        let kid_starts: Vec<usize> = (0..=children.len()).collect();
        let children = crate::layout::multicol::container::insert_static_probes(
            &self.merged,
            &opts,
            statics,
            spots,
            &kid_starts,
            children,
        );
        (children, geometry)
    }
}
