//! Запрос раскладки стопки колонок до отрисовки: замер высоты каждого ребёнка при ширине
//! колонки, разбиение детей на страницы, внутренние размеры многоколоночника по
//! css-sizing-4 §multicol-intrinsic (как Blink `ComputeMinMaxSizes`) для случаев, когда
//! ширину решает содержимое, и пробная балансировка, дающая размер коробки и базовые линии.

use crate::layout::fragment::plan::Plan;
use crate::layout::fragment::types::Intrinsic;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, LayoutId, Window, px, size};

mod breaks;
mod intrinsic;

impl ColumnStack {
    /// Раскладка детей идёт отдельным малым кадром: копия ребёнка может сама
    /// нести многоколоночник, чья раскладка вложена в эту, и большой кадр
    /// запроса меры (`request_measured`) на это время на стеке не лежит.
    pub(crate) async fn request_column_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, LayoutId) {
        self.lay_out_children_async(window, cx, stack).await;
        self.request_measured_async(window, cx, stack).await
    }

    #[inline(never)]
    async fn lay_out_children_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        if self.deferred.is_none() {
            for i in 0..self.children.len() {
                if !self.resume_nested_async(i, window, cx, stack).await {
                    self.lay_out_main_async(i, window, cx, stack).await;
                }
            }
            self.regrow_children_async(window, cx, stack).await;
            self.paginate_children_async(window, cx, stack).await;
            self.reserve_overflow_copies();
            self.report_lead();
        }
    }

    #[inline(never)]
    async fn request_measured_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, LayoutId) {
        // Распорки роста по раскладке копий — до разметки по сетке: её рамки
        // берут план с уже выросшими соседями.
        let heights = self.kid_geoms();
        let count = self.count;
        let fixed = self.fixed_height;
        let fill_shrink = self.fill_shrink;
        let gap = self.gap;
        let rows = self.rows;
        let copies = self.copies;
        let axis = self.axis;
        let row_phase = self.row_phase;
        let row_limit = self.row_limit;
        let row_extent = self.row_extent;
        let forced_h = self.forced_h;
        let balance_failure = self.balance_failure;
        let oof = self.oof_for_measurement();
        // Внутренние размеры многоколоночного контейнера. Спека их не
        // определяет (css-multicol-1 §3.4: «This specification does not
        // define how U is calculated»), единственное письменное определение —
        // css-sizing-4 `intrinsic-sizing-notes.bs` §multicol-intrinsic; его же
        // держит Blink (`column_layout_algorithm.cc:433`
        // `ComputeMinMaxSizes`). Прежде замер отдавал НОЛЬ, и всякий
        // многоколоночник, чью ширину решает содержимое (плавающий, строчная
        // коробка, элемент гибкого контейнера или сетки), схлопывался в
        // отбивку: `intrinsic-size-001` — зелёная коробка 60×100 вместо
        // 100×100, эталоны `column-grid-lanes-container-baseline-*` — полоса
        // в 25 px вместо 320.
        //
        // Детей меряем ТОЛЬКО когда ширину решает содержимое: обычному
        // блочному контейнеру её даёт родитель, и второй проход раскладки там
        // ничего не даст, кроме времени.
        let intrinsic = self.intrinsic_widths(window, cx, stack).await;
        let mut baselines = if !axis.is_vertical() && self.deferred.is_none() {
            Some(
                crate::layout::multicol::column_baselines::Measurements::new_async(
                    &mut self.children,
                    window,
                    cx,
                    stack,
                )
                .await,
            )
        } else {
            None
        };
        let paint = self.paint.clone();
        let deferred = self.deferred.clone();
        let nest_report = self.nest_report.clone();
        let lead_report = self.lead_report.clone();
        let discard = self.discard;
        let replay = self.replay;
        let id = window.request_measured_layout_with_baselines_async(
            gpui::Style::default(),
            async move |known,
                        available,
                        window: &mut Window,
                        cx: &mut App,
                        stack: &mut gpui::PaintStack| {
                let mut probe = ColumnStack {
                    paint: paint.clone(),
                    children: Vec::new(),
                    deferred: None,
                    count,
                    gap,
                    axis,
                    row_phase,
                    row_limit,
                    row_extent,
                    fixed_height: fixed,
                    forced_h,
                    balance_failure,
                    fill_shrink,
                    rule_to: None,
                    rows,
                    copies,
                    gap_items: None,
                    intrinsic: None,
                    discard,
                    replay,
                    plan: std::cell::RefCell::new(Plan::default()),
                    static_places: Default::default(),
                    oof: oof.fresh(),
                    col_w: std::cell::Cell::new(0.0),
                    lines_plan: std::cell::RefCell::new(Vec::new()),
                    spans_plan: std::cell::RefCell::new(Vec::new()),
                    nest_report: None,
                    lead_report: lead_report.clone(),
                };
                // Вертикальное письмо: место под прогрессию колонок — по
                // СТРОЧНОЙ оси, то есть высота коробки (css-multicol-1 §2), а
                // отдаём (блочный, строчный) как (ширина, высота).
                if axis.is_vertical() {
                    let inline = known
                        .height
                        .map(f32::from)
                        .or(match available.height {
                            gpui::AvailableSpace::Definite(v) => Some(f32::from(v)),
                            _ => None,
                        })
                        .unwrap_or(0.0);
                    probe
                        .col_w
                        .set(((inline - gap * (count as f32 - 1.)) / count as f32).max(1.));
                    let block = if let Some(recipe) = &deferred {
                        recipe
                            .measure_async(&mut probe, inline, window, cx, stack)
                            .await
                            .0
                    } else {
                        probe.balance(&heights).0
                    };
                    return (size(px(block), px(inline)), None, None);
                }
                let w = known
                    .width
                    .map(f32::from)
                    .or(match available.width {
                        gpui::AvailableSpace::Definite(v) => Some(f32::from(v)),
                        gpui::AvailableSpace::MinContent => intrinsic.map(|(mn, _)| mn),
                        gpui::AvailableSpace::MaxContent => intrinsic.map(|(_, mx)| mx),
                    })
                    .unwrap_or(0.0);
                if let Some(recipe) = &deferred {
                    let (height, first, last) =
                        recipe.measure_async(&mut probe, w, window, cx, stack).await;
                    return (size(px(w), px(height)), first, last);
                }
                probe
                    .col_w
                    .set(((w - gap * (count as f32 - 1.)) / count as f32).max(1.));
                let (height, lines, plan, spans) = probe.balance(&heights);
                if let Some(report) = &nest_report {
                    report.set(crate::layout::multicol::column_regrow::first_row_overflow(
                        &lines, &plan, count,
                    ));
                }
                let height = row_extent.block_size(height, &lines, &spans, &plan, count, &heights);
                let (first, last) = if let Some(baselines) = &mut baselines {
                    baselines
                        .measure_async(
                            &probe, &heights, w, &lines, &plan, &spans, window, cx, stack,
                        )
                        .await
                } else {
                    (None, None)
                };
                (size(px(w), px(height)), first, last)
            },
        );
        (id, id)
    }
}
