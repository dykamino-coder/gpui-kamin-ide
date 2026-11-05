//! Построение спаннеров и пробная геометрия используют общий стек обхода.
use super::*;
impl Recipe {
    pub(super) async fn build_async(
        &self,
        available: gpui::Size<AvailableSpace>,
        pass: SpannerPass,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
        stack: &mut gpui::PaintStack,
    ) -> AnyElement {
        let Self {
            e,
            merged,
            inherited,
            opts,
            outer,
            row_measures,
        } = self;
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
        let dimensions =
            MulticolDimensions::for_element(e, &style).with_measured_size(width, height);
        // План рядов читает собственные ограничения; стадия коробки
        // может разрешить процентную высоту иначе (quirks mode).
        let own = MulticolDimensions::for_element(&host, &host.style.used())
            .with_measured_size(width, height);
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
            opts,
        )
        .into();
        let plan = column_plan_in(&host, &style, &opts, dimensions, own);
        let count = plan.used_count.unwrap_or(1).max(1);
        let column_width = ((width + plan.used_gap) / f32::from(count) - plan.used_gap).max(0.);
        let mut probe_opts: RenderOpts = opts.clone();
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
                crate::layout::positioned::containing_block::metrics::layout::collect_async(
                    &kids,
                    &style,
                    column_width,
                    &probe_opts,
                    window,
                    cx,
                    stack,
                )
                .await,
            );
        }
        let mut measure =
            async |child: &Element, parent: &BoxStyle, width: f32, stack: &mut gpui::PaintStack| {
                probe::measure_async(child, parent, width, &probe_opts, window, cx, stack).await
            };
        if let Some(kids) = hoist_spanners_async(
            &host.children,
            &style,
            opts,
            column_width,
            &mut measure,
            stack,
        )
        .await
        {
            host.children = kids.into();
        }
        // Каждый ряд между спаннерами — свой набор колонок (css-multicol-1
        // §6.1; Blink `ColumnLayoutAlgorithm::LayoutRow`): исходный OOF-поток
        // ряда раскладывается от начала ряда и рисуется своей стопкой.
        let row_geometry = if probe_opts.cx.position.geometry.is_some() {
            let mut source_opts: RenderOpts = probe_opts.clone();
            source_opts.cx.position.geometry = None;
            let bodies = crate::layout::multicol::container::row_bodies(&host, is_span);
            let mut rows = Vec::with_capacity(bodies.len());
            for body in bodies {
                let kids: Vec<_> = body
                    .iter()
                    .filter_map(|n| match n {
                        Node::Element(e) => Some(e.clone()),
                        _ => None,
                    })
                    .collect();
                rows.push(Some(
                    crate::layout::positioned::containing_block::metrics::layout::collect_async(
                        &kids,
                        &style,
                        column_width,
                        &source_opts,
                        window,
                        cx,
                        stack,
                    )
                    .await,
                ));
            }
            Some((style.self_node, Rc::from(rows)))
        } else {
            None
        };
        let rows = row_plan_in(
            &host,
            opts,
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
                let used = window
                    .with_nested_layout_async(async |window| {
                        let mut el = build(
                            &prefix,
                            prefix_style,
                            inherited,
                            &probe_opts,
                            None,
                            None,
                            (dimensions.prefix(), own.prefix()),
                        );
                        f32::from(
                            el.layout_as_root_unrounded_async(
                                size(
                                    AvailableSpace::Definite(px(width)),
                                    AvailableSpace::MaxContent,
                                ),
                                window,
                                cx,
                                stack,
                            )
                            .await
                            .height,
                        )
                    })
                    .await;
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
        final_opts.cx.position.row_geometry = row_geometry;
        final_opts.cx.fragmentation.spanner_rows = RowSlot::build(
            row_measures,
            e.node_id,
            (width.to_bits(), height.map(f32::to_bits)),
            pass == SpannerPass::Measure,
            final_opts.frag.watch.is_some(),
        );
        build(
            &host,
            style,
            inherited,
            &final_opts,
            outer.clone(),
            rest,
            (dimensions, own),
        )
    }
}
