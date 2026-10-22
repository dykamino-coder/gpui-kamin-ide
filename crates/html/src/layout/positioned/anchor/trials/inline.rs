//! Строчные входы передают отдельные построения кандидатов живому хосту CB.

use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::containing_block::{
    OofKid, Spot, SpotCell, nearest_cb_index, spot_probe,
};
use crate::layout::positioned::predicates::edge_set;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Position};
use gpui::{AnyElement, IntoElement};

pub(crate) fn is_root(e: &Element, opts: &RenderOpts) -> bool {
    opts.cx
        .oof_trial
        .as_ref()
        .is_some_and(|(key, _)| *key == super::super::key_of(e))
}

pub(crate) fn defer(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    if !super::super::resolve::needs_deferred(&e.style)
        || is_root(e, opts)
        || opts.paged_build().is_some()
        || !crate::layout::order::hoists_inline_abs(e, &opts.cx)
    {
        return None;
    }
    let fixed = e.style.position == Some(Position::Fixed);
    let under_tf = inherited.transform_ancestor || inherited.transform.is_some();
    let to_icb = (fixed && !under_tf)
        || !(fixed || inherited.cb_ancestor || crate::text::inline::establishes_cb(inherited));
    let nearest = nearest_cb_index(inherited);
    if !opts.cx.positioning.can_defer(fixed, nearest, to_icb) {
        return None;
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
        own_vertical: e.style.wm().vertical(),
        replaced: matches!(
            e.tag.as_str(),
            "img" | "svg" | "iframe" | "canvas" | "video" | "object" | "embed"
        ),
        fixed_axes: (
            edge_set(e.style.inset.left) || edge_set(e.style.inset.right),
            edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom),
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
    let kid = OofKid::Trials(
        Box::new(move |bounds, spot, place, window, cx| {
            let vcx = build_opts.cx.vcx();
            let mut base = inherit(&inherited, &source.style, vcx);
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
                    super::apply_style(&mut candidate.style, style);
                    let child =
                        crate::layout::atom::positioned::trial(&inherited, &candidate, &opts)
                            .unwrap_or_else(|| gpui::div().into_any_element());
                    let child = crate::paint::stacking::inline_abs::inline_abs_paint_last(
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
    opts.cx
        .positioning
        .defer(spot, kid, fixed, nearest, to_icb)
        .ok()?;
    use gpui::{ParentElement, Styled};
    Some(gpui::div().w_0().h_0().child(probe).into_any_element())
}
