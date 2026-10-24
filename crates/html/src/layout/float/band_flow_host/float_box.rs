//! Независимое горизонтальное содержимое float, границы clamp и эффекты paint.

use crate::dom::Element;
use crate::layout::build_contracts::Effect;
use crate::layout::options::RenderOpts;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::BoxStyle;
use crate::text::clamp::{close_clamp_span, open_clamp_span};
use gpui::{AnyElement, IntoElement, ParentElement};

pub(super) fn float_box(
    copy: &Element,
    source: &Element,
    cb: f32,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let mut merged = box_style(inherited, &copy.style, vcx);
    merged.self_node = copy.node_id;
    merged.anchor_seq = opts.cx.anchor_seq_for(copy);
    merged.anchor_key = crate::layout::positioned::anchor::key_of(copy);
    merged.margin = Default::default();
    // Like element(), resolve viewport units before the box's native layout.
    merged.resolve_viewport(opts.viewport, vcx);
    let el = if copy.style.used().clamp_lines().is_some() || copy.style.used().clamp_by_height() {
        // A float can own a clamp context as well as isolate its parent's
        // counter. The native box installs that context, its cut and probes;
        // inheriting the paragraph's line limit alone loses the root strut.
        opts.cx.contracts.build.element(copy, inherited, opts)
    } else if copy.tag == "img" {
        opts.cx.contracts.build.replaced(copy, None, &opts.cx)
    } else {
        // The independent float contents retain the same positioned containing
        // block as an ordinary box; its OOF host follows the in-flow anchors.
        let cb_scope = crate::text::inline::establishes_cb(&merged).then(|| {
            crate::layout::positioned::layer_scope::LayerScope::cb(
                &opts.cx.position.state,
                crate::style::computed::stacking::fixed_cb_layer_box(&merged),
            )
        });
        let span = open_clamp_span(&opts.cx);
        let mut kids = opts
            .cx
            .contracts
            .build
            .blocks(&copy.children, &merged, opts);
        close_clamp_span(span.clone(), &opts.cx);
        if let Some(scope) = cb_scope {
            kids.extend(scope.finish());
        }
        // CSS Overflow 4 section 5.3: its lines belong to an independent BFC,
        // but a float intersecting the clamp point keeps its border-box extent.
        let mut probed = merged.clone();
        probed.float = source.style.used().float;
        // The host owns placement margins; the clamp probe still needs the
        // float's non-collapsing end margin for the containing BFC's extent.
        probed.margin.bottom = match source.style.used().margin.bottom {
            Some(crate::style::values::value::Len::Pct(k)) => {
                Some(crate::style::values::value::Len::Px(k * cb))
            }
            margin => margin,
        };
        opts.cx
            .contracts
            .build
            .probes(copy, &probed, opts, &mut kids, true, false, span);
        opts.cx
            .contracts
            .build
            .styled_box(copy, &merged, &opts.cx)
            .children(kids)
            .into_any_element()
    };
    // CSS Transforms 1 section 3: placement keeps the untransformed rectangle.
    let build = opts.cx.contracts.build;
    let el = build.effects(el, Effect::transform(&merged, inherited, vcx));
    build.effects(el, Effect::group(&copy.style.used(), vcx))
}
