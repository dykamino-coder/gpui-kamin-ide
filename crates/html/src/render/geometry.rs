//! Адаптер запросов геометрии: наблюдатели ставятся только отдельным построителем.

use crate::{
    dom::Element,
    layout::{
        build_contracts::ChildBuild,
        cx::LayoutCx,
        geometry::{Collector, LineHeight},
    },
    style::computed::BoxStyle,
};
use gpui::{Bounds, LayoutId, LayoutObserver, Pixels, Window};

pub(crate) fn builder() -> ChildBuild {
    ChildBuild::default().for_geometry(super::paragraph::paragraph_for::<true>, styled_box)
}

fn styled_box(
    e: &Element,
    style: &BoxStyle,
    cx: &LayoutCx,
    dimensions: Option<crate::layout::block::sizing::BoxDimensions>,
) -> gpui::Div {
    crate::paint::box_tree::styled::styled_div_with_dimensions(e, style, cx, dimensions)
        .also_observe_layout(Observe {
            node: e.node_id,
            font: crate::layout::geometry::font(style, 16., cx.vcx()),
            layout: None,
        })
}

struct Observe {
    node: u64,
    font: (f64, LineHeight),
    layout: Option<LayoutId>,
}

impl LayoutObserver for Observe {
    fn request_layout(&mut self, id: LayoutId, _: &Window) {
        self.layout = Some(id);
    }
    fn prepaint(&mut self, window: &mut Window) -> bool {
        if let Some(id) = self.layout {
            let b = Bounds::new(
                window.layout_origin_unrounded(id),
                window.layout_size_unrounded(id),
            );
            let (border, padding) = window.layout_box_edges(id);
            let p = inset(b, border);
            Collector::record(window, self.node, [b, p, inset(p, padding)], self.font);
        }
        true
    }
}

fn inset(b: Bounds<Pixels>, e: gpui::Edges<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        b.origin + gpui::point(e.left, e.top),
        gpui::size(
            (b.size.width - e.left - e.right).max(gpui::px(0.)),
            (b.size.height - e.top - e.bottom).max(gpui::px(0.)),
        ),
    )
}
