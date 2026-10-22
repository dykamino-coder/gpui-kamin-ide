//! Layout-only пробы с собственными сборщиками; prepaint выполняет хост победителя.

use crate::document::state::ValueCx;
use crate::layout::positioned::anchor::trials::{Options, Probe};
use crate::layout::positioned::containing_block::{Escaped, SpotCell};
use crate::style::computed::Computed;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, IntoElement, ParentElement, Pixels, Styled, Window,
    size,
};
use std::rc::Rc;

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    bounds: Bounds<Pixels>,
    spot: SpotCell,
    place: fn(SpotCell, AnyElement) -> AnyElement,
    options: &Options,
    inherited: &Computed,
    mut build: impl FnMut(&Computed, Rc<Probe>, &mut Window) -> (AnyElement, Escaped),
    window: &mut Window,
    cx: &mut App,
    vcx: ValueCx<'_>,
) -> AnyElement {
    let mut roots = Vec::new();
    let mut probes = Vec::new();
    for (_, style) in &options.styles {
        let probe = Rc::new(Probe::default());
        let mut now = spot.get();
        use crate::layout::positioned::predicates::edge_set;
        now.fixed_axes = (
            edge_set(style.inset.left) || edge_set(style.inset.right),
            edge_set(style.inset.top) || edge_set(style.inset.bottom),
        );
        now.free_margin = (
            if now.fixed_axes.0 {
                0.
            } else {
                crate::layout::block::struts::margin_px(style.margin.left, style, vcx).unwrap_or(0.)
            },
            if now.fixed_axes.1 {
                0.
            } else {
                crate::layout::block::struts::margin_px(style.margin.top, style, vcx).unwrap_or(0.)
            },
        );
        now.self_align =
            crate::layout::positioned::static_position::static_self_align_style(style, inherited);
        probe.spot.set(now);
        let (child, escaped) = build(style, probe.clone(), window);
        let mut root = gpui::div()
            .relative()
            .w(bounds.size.width)
            .h(bounds.size.height)
            .child(place(probe.spot.clone(), child))
            .into_any_element();
        root.layout_as_root_at(
            bounds.origin,
            size(
                AvailableSpace::Definite(bounds.size.width),
                AvailableSpace::Definite(bounds.size.height),
            ),
            window,
            cx,
        );
        roots.push((root, escaped));
        probes.push(probe);
    }
    let chosen = options.choose(&probes, window, vcx);
    options.record(chosen, vcx);
    let (root, escaped) = roots.swap_remove(chosen);
    escaped.commit();
    root
}
