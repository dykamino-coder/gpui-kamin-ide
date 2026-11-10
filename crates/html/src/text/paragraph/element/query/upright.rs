//! Физическая inline-коробка стопки: одна колонка, а не коробка на каждый глиф.

use crate::{layout::geometry::Collector, text::paragraph::*};
use gpui::{Bounds, Pixels, Window, point, px, size};

impl Paragraph {
    pub(super) fn collect_upright_geometry(
        &self,
        segs: &[Seg],
        range: std::ops::Range<usize>,
        info: &crate::text::inline::inline_box::InlineBoxInfo,
        shift: (f32, f32),
        bounds: Bounds<Pixels>,
        window: &Window,
    ) {
        let Some((values, _)) = &self.anchor_values else {
            return;
        };
        let source = Collector::source_style(window, info.node);
        let style = source.as_deref().unwrap_or(&info.font_style);
        let metrics = self.inline_font_metrics(info, window);
        let mut content: Option<Bounds<Pixels>> = None;
        self.visit_inline_fragments(
            segs,
            range,
            [0.; 4],
            metrics,
            bounds,
            |_, _, (top, advance)| {
                let physical = Bounds::new(
                    point(bounds.center().x - px((metrics.0 + metrics.1) / 2.), top),
                    size(px(metrics.0 + metrics.1), advance),
                );
                content = Some(content.map_or(physical, |old| old.union(&physical)));
            },
        );
        let Some(mut content) = content else { return };
        content.origin += point(px(shift.0), px(shift.1));
        let expand = |r: Bounds<Pixels>, edges: crate::style::computed::Sides| {
            let [top, right, bottom, left] = [edges.top, edges.right, edges.bottom, edges.left]
                .map(crate::style::values::value::px_lossy_zero)
                .map(px);
            Bounds::new(
                r.origin - point(left, top),
                size(r.size.width + left + right, r.size.height + top + bottom),
            )
        };
        let padding = expand(content, style.reset_box_layout.padding);
        let border = expand(padding, style.borders());
        Collector::record(
            window,
            info.node,
            [border, padding, content],
            crate::layout::geometry::font(style, f32::from(self.font_size), values.vcx()),
        );
    }
}
