//! Исходная OOF-геометрия для отложенного многоколоночника после определения ширины.

use super::*;

impl Recipe {
    pub(super) fn geometry_options(
        &self,
        host: &Element,
        width: f32,
        host_box: Option<(f32, Option<f32>)>,
        window: &mut Window,
        cx: &mut App,
    ) -> RenderOpts {
        let mut opts = self.opts.clone();
        if !self.axis.is_vertical()
            && self.rows.is_none()
            && opts.cx.position.geometry_record.is_none()
        {
            opts.cx.position.geometry = Some(
                crate::layout::positioned::containing_block::metrics::layout::collect_nodes(
                    &host.children,
                    &self.merged,
                    width,
                    host_box,
                    &opts,
                    window,
                    cx,
                ),
            );
        }
        opts
    }
}
