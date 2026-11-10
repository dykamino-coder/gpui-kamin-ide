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
    ChildBuild::default().for_geometry(
        super::paragraph::paragraph_for::<true>,
        styled_box,
        super::element::element_for::<true>,
        |stk, e, style, opts| {
            Box::pin(super::element::element_stack_for::<true>(
                stk, e, style, opts,
            ))
        },
        svg::build,
        effects,
        crate::paint::effects::grouped::paint_group::<true>,
    )
}

mod svg;

fn effects(
    child: gpui::AnyElement,
    effect: crate::layout::build_contracts::Effect<'_>,
) -> gpui::AnyElement {
    use crate::layout::build_contracts::Effect;
    match effect {
        Effect::Transform {
            node,
            style,
            parent,
            ref_box,
            frame,
        } => crate::paint::effects::transform::transformed_inner::<true>(
            child, style, parent, ref_box, node, frame,
        ),
        Effect::ElementTransform {
            element,
            parent,
            frame,
        } => {
            let style =
                crate::style::cascade::materialize::box_style(parent, &element.style, frame.values);
            crate::paint::effects::transform::transformed_inner::<true>(
                child,
                &style,
                parent,
                None,
                Some(element.node_id),
                frame,
            )
        }
        other => super::child_builder::effects(child, other),
    }
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
            let content = inset(p, padding);
            Collector::record(window, self.node, [b, p, content], self.font);
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
