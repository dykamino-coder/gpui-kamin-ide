//! Independent horizontal float contents, clamp boundaries and paint effects.

use crate::dom::Element;
use crate::layout::replaced::image::image;
use crate::paint::effects::grouped::grouped;
use crate::paint::effects::transform::transformed;
use crate::render::{RenderOpts, blocks, styled_div_with};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::text::clamp::{close_clamp_span, open_clamp_span};
use gpui::{AnyElement, IntoElement, ParentElement};

pub(super) fn float_box(
    copy: &Element,
    source: &Element,
    cb: f32,
    inherited: &Computed,
    opts: &RenderOpts,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let mut merged = inherit(inherited, &copy.style, vcx);
    merged.self_node = copy.node_id;
    merged.margin = Default::default();
    // Like element(), resolve viewport units before the box's native layout.
    merged.resolve_viewport(opts.viewport, vcx);
    let el = if copy.tag == "img" {
        image(copy, &opts.cx)
    } else {
        let span = open_clamp_span(&opts.cx);
        let mut kids = blocks(&copy.children, &merged, opts);
        close_clamp_span(span.clone());
        // CSS Overflow 4 section 5.3: its lines belong to an independent BFC,
        // but a float intersecting the clamp point keeps its border-box extent.
        let mut probed = merged.clone();
        probed.float = source.style.float;
        // The host owns placement margins; the clamp probe still needs the
        // float's non-collapsing end margin for the containing BFC's extent.
        probed.margin.bottom = match source.style.margin.bottom {
            Some(crate::style::values::value::Len::Pct(k)) => {
                Some(crate::style::values::value::Len::Px(k * cb))
            }
            margin => margin,
        };
        crate::render::generic_box::child_probes(copy, &probed, opts, &mut kids, true, false, span);
        styled_div_with(copy, &merged, &opts.cx)
            .children(kids)
            .into_any_element()
    };
    // CSS Transforms 1 section 3: placement keeps the untransformed rectangle.
    grouped(transformed(el, &merged, inherited), &copy.style, vcx)
}
