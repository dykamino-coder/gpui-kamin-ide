//! Исходная OOF-геометрия для отложенного многоколоночника после определения ширины.

use super::*;

impl Recipe {
    pub(super) fn geometry_options(
        &self,
        host: &Element,
        width: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> RenderOpts {
        let mut opts = self.opts.clone();
        if !self.axis.is_vertical()
            && self.rows.is_none()
            && self.fixed.is_some_and(|h| h > crate::tolerance::COORD_EPS)
            && opts.cx.position.geometry_record.is_none()
            // Inside a copy of an outer native source its flows own these boxes.
            && !opts.cx.position.continuation_slots
        {
            let source = host
                .children
                .iter()
                .filter_map(|n| match n {
                    Node::Element(c) => Some(c.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            opts.cx.position.geometry = Some(
                crate::layout::positioned::containing_block::metrics::layout::collect(
                    &source,
                    &self.merged,
                    width,
                    &opts,
                    window,
                    cx,
                ),
            );
        }
        opts
    }
}
