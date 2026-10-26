//! Адаптер spanner path: native-геометрия при разрешённой ширине и расход
//! внешнего ряда. Временные LayoutId не переживают вложенный layout.

use super::super::columns::column_plan_in;
use super::settled;
use crate::layout::multicol::dimensions::MulticolDimensions;
mod intrinsic;
mod probe;
use crate::dom::{Element, Node};
use crate::layout::fragment::token::space::sequence::Nested;
use crate::layout::multicol::measure::{SpannerRow, row_plan_in, tail_rest_height};
use crate::layout::multicol::spanner::{SpannerFlow, SpannerPass, hoist_spanners, spanner_box};
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;
use gpui::{AnyElement, AvailableSpace, IntoElement, Styled, div, px, size};
use std::rc::Rc;

pub(super) fn deferred(
    e: &Element,
    merged: &BoxStyle,
    inherited: &Computed,
    opts: &RenderOpts,
    outer: Option<Rc<Nested>>,
) -> AnyElement {
    let mut opts = opts.clone();
    opts.cx.position.state = opts.cx.position.state.retain().resume();
    let (e, merged, inherited) = (e.clone(), merged.clone(), inherited.clone());
    let (intrinsic_e, intrinsic_style, intrinsic_opts) = (e.clone(), merged.clone(), opts.clone());
    SpannerFlow::deferred(
        move |available, pass, window, cx| {
            let AvailableSpace::Definite(width) = available.width else {
                unreachable!("resolved spanner width")
            };
            let width = f32::from(width);
            let mut host = e.clone();
            let style = merged.clone();
            let height = match available.height {
                AvailableSpace::Definite(h) => Some(f32::from(h)),
                _ => None,
            };
            let dimensions = MulticolDimensions::measured(&style, width, height);
            // План рядов читает собственные ограничения; стадия коробки
            // может разрешить процентную высоту иначе (quirks mode).
            let own = MulticolDimensions::measured(&host.style.used(), width, height);
            host.children = crate::layout::fragment::flow::column_percentages_in(
                &host.children,
                &style,
                dimensions.percentage_height(
                    &style,
                    opts.cx.constraints.avail_h,
                    opts.cx.constraints.avail_w,
                    opts.viewport.1,
                ),
                Some(width),
                &opts,
            )
            .into();
            let plan = column_plan_in(&host, &style, &opts, dimensions, own);
            let count = plan.used_count.unwrap_or(1).max(1);
            let column_width = ((width + plan.used_gap) / f32::from(count) - plan.used_gap).max(0.);
            let mut probe_opts = opts.clone();
            probe_opts.frag.watch = None;
            probe_opts.cx.position.state = Default::default();
            probe_opts.cx.deferred = Default::default();
            if opts.cx.position.geometry_record.is_none() && opts.cx.position.geometry.is_none() {
                let kids: Vec<_> = host
                    .children
                    .iter()
                    .filter_map(|n| match n {
                        Node::Element(e) => Some(e.clone()),
                        _ => None,
                    })
                    .collect();
                probe_opts.cx.position.geometry = Some(
                    crate::layout::positioned::containing_block::metrics::layout::collect(
                        &kids,
                        &style,
                        column_width,
                        &probe_opts,
                        window,
                        cx,
                    ),
                );
            }
            let mut measure = |child: &Element, parent: &BoxStyle, width: f32| {
                probe::measure(child, parent, width, &probe_opts, window, cx)
            };
            if let Some(kids) =
                hoist_spanners(&host.children, &style, &opts, column_width, &mut measure)
            {
                host.children = kids.into();
            }
            let rows = row_plan_in(
                &host,
                &opts,
                plan.column_width,
                count,
                plan.col_vert,
                outer.clone(),
                own,
            );
            let rest = match (
                rows.box_h,
                rows.rows.is_none_or(|r| r.cap),
                host.children.iter().rposition(is_span),
            ) {
                (Some(total), true, Some(last)) => {
                    let mut prefix = host.clone();
                    prefix.children.truncate(last + 1);
                    let prefix_style = style.clone();
                    let used = window.with_nested_layout(|window| {
                        let mut el = build(
                            &prefix,
                            prefix_style,
                            &inherited,
                            &probe_opts,
                            None,
                            None,
                            (dimensions.prefix(), own.prefix()),
                        );
                        f32::from(
                            el.layout_as_root_unrounded(
                                size(
                                    AvailableSpace::Definite(px(width)),
                                    AvailableSpace::MaxContent,
                                ),
                                window,
                                cx,
                            )
                            .height,
                        )
                    });
                    let phase = SpannerRow::after_spanner(last, used)
                        .expect("finite completed spanner row geometry");
                    Some(tail_rest_height(total, &phase))
                }
                _ => None,
            };
            let mut final_opts: RenderOpts = if pass == SpannerPass::Final {
                opts.clone()
            } else {
                probe_opts.clone()
            };
            final_opts.cx.position.geometry = probe_opts.cx.position.geometry;
            build(
                &host,
                style,
                &inherited,
                &final_opts,
                outer.clone(),
                rest,
                (dimensions, own),
            )
        },
        move |available, window, cx| {
            intrinsic::measure(
                &intrinsic_e,
                &intrinsic_style,
                &intrinsic_opts,
                available,
                window,
                cx,
            )
        },
    )
}

fn is_span(n: &Node) -> bool {
    matches!(n, Node::Element(e) if spanner_box(e))
}

#[allow(clippy::too_many_arguments)]
fn build(
    host: &Element,
    style: BoxStyle,
    inherited: &Computed,
    opts: &RenderOpts,
    outer: Option<Rc<Nested>>,
    rest: Option<f32>,
    (dimensions, own): (MulticolDimensions, MulticolDimensions),
) -> AnyElement {
    let p = column_plan_in(host, &style, opts, dimensions, own);
    let mut d = div();
    let vertical = style.wm().vertical();
    let axes = style.wm().reversed_axes();
    d.style().block_flow = Some([vertical, vertical && axes[0], axes[usize::from(vertical)]]);
    // Размер анонимного потока задаёт CSS-хост; декорации остаются снаружи.
    if let Some(Len::Px(h)) = dimensions.preferred[1] {
        d = d.h(px(h));
    }
    match settled(
        d,
        host,
        style,
        inherited,
        opts,
        p.column_width,
        p.used_count.unwrap_or(0),
        p.used_gap,
        p.col_axis,
        p.col_vert,
        p.col_rl,
        p.col_inline_size,
        outer,
        p.lone_span,
        rest,
        Some((dimensions, own)),
    ) {
        Ok((d, _)) => d.into_any_element(),
        Err(el) => el,
    }
}
