//! Строчные входы передают отдельные построения кандидатов живому хосту CB.

use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::containing_block::{
    OofKid, Spot, SpotCell, nearest_cb_index, spot_probe,
};
use crate::layout::positioned::predicates::edge_set;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{BoxStyle, Position};
use gpui::{AnyElement, IntoElement};

pub(crate) fn is_root(e: &Element, opts: &RenderOpts) -> bool {
    opts.cx
        .oof_trial
        .as_ref()
        .is_some_and(|(key, _)| *key == super::super::key_of(e))
}

pub(crate) fn defer(e: &Element, inherited: &BoxStyle, opts: &RenderOpts) -> Option<AnyElement> {
    if !super::super::resolve::needs_host(&e.style.computed(), &opts.cx)
        || is_root(e, opts)
        || opts.paged_build().is_some()
    {
        return None;
    }
    let fixed = e.style.computed().position == Some(Position::Fixed);
    let under_tf = inherited.transform_ancestor || inherited.transform.is_some();
    let to_icb = (fixed && !under_tf)
        || !(fixed || inherited.cb_ancestor || crate::text::inline::establishes_cb(inherited));
    let nearest = nearest_cb_index(inherited);
    let hoist = crate::layout::order::hoists_inline_abs(e, &opts.cx)
        && opts.cx.positioning.can_defer(fixed, nearest, to_icb);
    let inline_cb = !hoist
        && opts.cx.inline_cb.as_ref().is_some_and(|o| o.take())
        && !fixed
        && (edge_set(e.style.computed().inset.left) || edge_set(e.style.computed().inset.right))
        && (edge_set(e.style.computed().inset.top) || edge_set(e.style.computed().inset.bottom))
        && e.style.computed().z_index.unwrap_or(0) >= 0;
    if inline_cb && let Some(offer) = &opts.cx.inline_cb {
        offer.note_taken();
    }
    let spot: SpotCell = Default::default();
    let block = !crate::layout::positioned::static_position::hypothetical_inline_level(e);
    spot.set(Spot {
        next_line: block.then(|| crate::text::text_box::line_height_px(inherited, opts)),
        line_rect: true,
        line_thickness: crate::text::text_box::line_height_px(inherited, opts),
        preserve_cb_geometry: !to_icb,
        viewport_static: to_icb,
        rtl: inherited.wm().rtl(),
        vertical: inherited.wm().vertical(),
        vertical_rl: inherited.wm().block_rl(),
        own_vertical: e.style.computed().wm().vertical(),
        replaced: matches!(
            e.tag.as_str(),
            "img" | "svg" | "iframe" | "canvas" | "video" | "object" | "embed"
        ),
        fixed_axes: (
            edge_set(e.style.computed().inset.left) || edge_set(e.style.computed().inset.right),
            edge_set(e.style.computed().inset.top) || edge_set(e.style.computed().inset.bottom),
        ),
        ..Default::default()
    });
    let probe = spot_probe(spot.clone(), false, opts.cx.line_box.clone(), &opts.cx);
    let mut build_opts = opts.clone();
    let key = opts.cx.paint_key_for(e);
    build_opts.cx.oof_identities = Some(opts.cx.retain_oof_identities(e, key));
    let retained = opts.cx.positioning.retain();
    let source = e.clone();
    let inherited = inherited.clone();
    let vcx = opts.cx.vcx();
    let mut own = box_style(&inherited, &source.style, vcx);
    own.self_node = source.node_id;
    own.anchor_key = super::super::key_of(&source);
    own.anchor_seq = build_opts.cx.anchor_seq_for(&source);
    let cb_node = own.cb_node;
    let dependencies = super::super::Dependencies::of(&own, &inherited, vcx);
    let kid = OofKid::Trials(
        Box::new(move |bounds, spot, place, window, cx| {
            let vcx = build_opts.cx.vcx();
            let mut base = box_style(&inherited, &source.style, vcx);
            base.self_node = source.node_id;
            base.anchor_key = super::super::key_of(&source);
            base.anchor_seq = build_opts.cx.anchor_seq_for(&source);
            base.resolve_viewport(build_opts.viewport, vcx);
            let options = super::Options::new(&base, &inherited, vcx);
            super::run(
                bounds,
                spot,
                place,
                &options,
                &inherited,
                |style, trial, window| {
                    let (positioning, escaped) = retained.isolate();
                    let mut opts = build_opts.clone();
                    opts.cx.positioning = positioning;
                    opts.cx.oof_trial = Some((base.anchor_key, trial));
                    opts.cx.oof_sizes = Some((
                        base.anchor_key,
                        super::super::resolve::current_inputs(
                            style, &inherited, bounds, window, vcx,
                        ),
                    ));
                    let mut candidate = source.clone();
                    super::apply_style(candidate.style.used_mut(), style);
                    let child =
                        crate::layout::atom::positioned::trial(&inherited, &candidate, &opts)
                            .unwrap_or_else(|| gpui::div().into_any_element());
                    let child = crate::layout::atom::positioned::inline_abs_paint_last(
                        &candidate, child, &opts.cx,
                    );
                    (child, escaped)
                },
                window,
                cx,
                vcx,
            )
        }),
        None,
    );
    let kid = OofKid::Waiting(dependencies, Box::new(kid));
    use gpui::{ParentElement, Styled};
    if hoist {
        opts.cx
            .positioning
            .defer(spot, kid, fixed, nearest, to_icb)
            .ok()?;
        Some(gpui::div().w_0().h_0().child(probe).into_any_element())
    } else {
        let host = opts
            .cx
            .positioning
            .local_oof(spot, kid, cb_node, opts.cx.vcx().keep());
        if inline_cb {
            Some(host)
        } else {
            Some(
                gpui::div()
                    .w_0()
                    .h_0()
                    .child(probe)
                    .child(host)
                    .into_any_element(),
            )
        }
    }
}
