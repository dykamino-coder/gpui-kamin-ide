//! Передача anchor-зависимого OOF хосту до animated и ChildBuild.element.

use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::anchor::{key_of, resolve};
use crate::layout::positioned::containing_block::{
    OofKid, Spot, SpotCell, nearest_cb_index, spot_probe,
};
use crate::layout::positioned::predicates::edge_set;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{BoxStyle, Position};
use gpui::{AnyElement, IntoElement};

use crate::layout::positioned::anchor::trials;

#[allow(clippy::too_many_arguments)]
pub(super) fn defer(
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    frame: &crate::interactive::sticky::element::StickyCell,
    layer_ok: bool,
    under_tf: bool,
    ordered: bool,
    geometry_ok: bool,
    paint_key: u64,
    root_box: Option<crate::paint::background::RootMeasurement>,
    out: &mut Vec<AnyElement>,
    rest: &[crate::dom::Node],
) -> bool {
    if !geometry_ok
        || opts.paged_build().is_some()
        || !resolve::needs_host(&e.style.used(), &opts.cx)
    {
        return false;
    }
    let fixed = e.style.used().position == Some(Position::Fixed);
    let to_icb = (fixed && !under_tf)
        || !(fixed || inherited.cb_ancestor || crate::text::inline::establishes_cb(inherited));
    let nearest = nearest_cb_index(inherited);
    if !opts.cx.position.state.can_defer(fixed, nearest, to_icb) {
        return false;
    }
    let axes = (
        edge_set(e.style.used().inset.left) || edge_set(e.style.used().inset.right),
        edge_set(e.style.used().inset.top) || edge_set(e.style.used().inset.bottom),
    );
    let spot: SpotCell = Default::default();
    spot.set(Spot {
        fixed_axes: axes,
        preserve_cb_geometry: !to_icb,
        viewport_static: to_icb,
        free_margin: (
            if axes.0 {
                0.
            } else {
                crate::layout::block::struts::margin_px(
                    e.style.used().margin.left,
                    &e.style.used(),
                    opts.cx.vcx(),
                )
                .unwrap_or(0.)
            },
            if axes.1 {
                0.
            } else {
                crate::layout::block::struts::margin_px(
                    e.style.used().margin.top,
                    &e.style.used(),
                    opts.cx.vcx(),
                )
                .unwrap_or(0.)
            },
        ),
        replaced: matches!(
            e.tag.as_str(),
            "img" | "iframe" | "video" | "canvas" | "object" | "embed" | "svg"
        ),
        rtl: inherited.wm().rtl(),
        vertical: inherited.wm().vertical(),
        vertical_rl: inherited.wm().block_rl(),
        own_vertical: e.style.used().wm().vertical(),
        self_align: crate::layout::positioned::static_position::static_self_align(e, inherited),
        ..Default::default()
    });
    let identities = opts.cx.retain_oof_identities(e, paint_key);
    let retained = opts.cx.position.state.retain();
    let source = e.clone();
    let inherited = inherited.clone();
    let mut build_opts = opts.clone();
    build_opts.cx.position.oof_identities = Some(identities);
    let frame = frame.clone();
    let paint_last = crate::layout::order::paint_last_ok(e, rest, &opts.cx);
    let source_paint = (!paint_last
        || crate::layout::positioned::predicates::stays_positioned(rest))
    .then(crate::layout::positioned::containing_block::DeferredPaint::default);
    let base_opts = build_opts.clone();
    let base_source = source.clone();
    let base_inherited = inherited.clone();
    let mut build = move |style: Option<&BoxStyle>,
                          positioning,
                          probe: Option<
        std::rc::Rc<crate::layout::positioned::anchor::trials::Probe>,
    >,
                          bounds,
                          window: &mut gpui::Window| {
        build_opts.cx.position.state = positioning;
        let mut candidate = source.clone();
        if let Some(style) = style {
            crate::layout::positioned::anchor::trials::apply_style(
                candidate.style.used_mut(),
                style,
            );
        }
        let source = &candidate;
        let vcx = build_opts.cx.vcx();
        let mut own = box_style(&inherited, &source.style, vcx);
        own.self_node = source.node_id;
        own.anchor_key = key_of(source);
        own.anchor_seq = build_opts.cx.anchor_seq_for(source);
        own.resolve_viewport(build_opts.viewport, vcx);
        let inputs = resolve::current_inputs(
            &own,
            &inherited,
            bounds,
            window,
            vcx,
            build_opts.cx.layout(),
        );
        build_opts.cx.position.oof_sizes = Some((own.anchor_key, inputs));
        build_opts.cx.position.oof_trial = probe.map(|p| (own.anchor_key, p));
        build_opts.cx.position.oof_computed = style
            .and_then(|s| s.element_style.clone())
            .map(|e| (own.anchor_key, e));
        let child = reblessive::Stack::new()
            .enter(|stk| {
                super::build::build(
                    stk,
                    source,
                    &inherited,
                    &build_opts,
                    &frame,
                    layer_ok,
                    root_box.clone(),
                )
            })
            .finish();
        let child = if !fixed && source.style.used().z_index.is_some_and(|z| z > 0) {
            crate::paint::stacking::layered(
                child,
                &source.style.used(),
                &inherited,
                layer_ok,
                under_tf,
            )
        } else {
            child
        };
        let child = if paint_last {
            crate::paint::coordinator::PaintLast::new(child)
                .key(paint_key)
                .into_any_element()
        } else {
            child
        };
        if source.style.used().z_index.is_some_and(|z| z < 0)
            && !crate::paint::stacking::nested_context(&inherited)
        {
            crate::hooks::IntoHooked::hooked(crate::paint::effects::underlay::Underlay::new(child))
        } else {
            child
        }
    };
    let vcx = opts.cx.vcx();
    let mut dependency_style = box_style(&base_inherited, &e.style, vcx);
    dependency_style.self_node = e.node_id;
    dependency_style.anchor_key = key_of(e);
    dependency_style.anchor_seq = base_opts.cx.anchor_seq_for(e);
    let dependencies = crate::layout::positioned::anchor::Dependencies::of(
        &dependency_style,
        &base_inherited,
        vcx,
        opts.cx.layout(),
    );
    let kid = if e.style.used().position_try_fallbacks.is_empty() {
        OofKid::Deferred(
            Box::new(move |bounds, window, _cx| {
                build(None, retained.resume(), None, bounds, window)
            }),
            source_paint.clone(),
        )
    } else {
        OofKid::Trials(
            Box::new(move |bounds, spot, place, window, cx| {
                let vcx = base_opts.cx.vcx();
                let mut base = box_style(&base_inherited, &base_source.style, vcx);
                base.self_node = base_source.node_id;
                base.anchor_key = key_of(&base_source);
                base.anchor_seq = base_opts.cx.anchor_seq_for(&base_source);
                base.resolve_viewport(base_opts.viewport, vcx);
                let options = crate::layout::positioned::anchor::trials::Options::new(
                    &base,
                    &base_inherited,
                    vcx,
                    base_opts.cx.layout(),
                );
                trials::run(
                    bounds,
                    spot,
                    place,
                    &options,
                    &base_inherited,
                    |style, probe, window| {
                        let (positioning, escaped) = retained.isolate();
                        let child = build(Some(style), positioning, Some(probe), bounds, window);
                        (child, escaped)
                    },
                    window,
                    cx,
                    vcx,
                )
            }),
            source_paint.clone(),
        )
    };
    if opts
        .cx
        .position
        .state
        .defer(
            spot.clone(),
            OofKid::Waiting(dependencies, Box::new(kid)),
            fixed,
            nearest,
            to_icb,
        )
        .is_err()
    {
        return false;
    }
    if !(ordered || axes.0 && axes.1) {
        out.push(spot_probe(spot, true, None, &opts.cx));
    }
    if let Some(paint) = source_paint {
        out.push(opts.cx.contracts.build.layer(
            crate::layout::build_contracts::Layer::DeferredPositioned(paint),
        ));
    }
    true
}
