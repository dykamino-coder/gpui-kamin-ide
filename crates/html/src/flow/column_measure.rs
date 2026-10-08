//! Native column measurement and intrinsic contributions before painting.

use super::*;

impl ColumnStack {
    pub(super) fn request_column_layout(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        for c in self.children.iter_mut() {
            if let Some(w) = c.measure {
                let (sz, content) = c.el.layout_as_root_with_content(
                    gpui::size(
                        gpui::AvailableSpace::Definite(gpui::px(w)),
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    cx,
                );
                let h = f32::from(sz.height);
                c.h = h;
                // Видимое переполнение потомков — параллельный поток
                // (css-break-3 §3): продолжается в следующих колонках само.
                c.over = h.max(f32::from(content.height));
            }
        }
        let heights: Vec<Kid> = self
            .children
            .iter()
            .map(|c| Kid {
                h: c.h,
                mt: c.mt,
                mb: c.mb,
                monolith: c.monolith,
                cuts: c.cuts.clone(),
                force_before: c.force_before,
                force_after: c.force_after,
                avoid_before: c.avoid_before,
                avoid_after: c.avoid_after,
                forced: c.forced.clone(),
                solid: c.solid.clone(),
                span: c.span,
                over: c.over,
                clone_dec: c.clone_dec,
                overflow_top: c.overflow_top,
                repeat: c.repeat.as_ref().map_or(RepeatGeom::default(), |r| r.geom),
                par: c.par,
            })
            .collect();
        let count = self.count;
        let fixed = self.fixed_height;
        let gap = self.gap;
        let rows = self.rows;
        let copies = self.copies;
        let axis = self.axis;
        let row_phase = self.row_phase;
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
        let intrinsic = self
            .intrinsic
            .filter(|_| !axis.is_vertical())
            .map(|Intrinsic(col_w)| {
                let (mut kid_min, mut kid_max) = (0.0f32, 0.0f32);
                let (mut span_min, mut span_max) = (0.0f32, 0.0f32);
                for c in self.children.iter_mut() {
                    let mut measure = |el: &mut AnyElement, w: gpui::AvailableSpace| {
                        intrinsic_measure::width(el, w, window, cx)
                    };
                    let mn = measure(&mut c.el, gpui::AvailableSpace::MinContent);
                    let mx = measure(&mut c.el, gpui::AvailableSpace::MaxContent);
                    // Спаннер идёт во всю ширину коробки: на число колонок он не
                    // умножается, а лишь ПОДНИМАЕТ итог — Blink
                    // `ComputeSpannersMinMaxSizes` (:523) через
                    // `MinMaxSizes::Encompass` (`min_max_sizes.h:26`, это `max`
                    // по обеим границам).
                    if c.span {
                        span_min = span_min.max(mn);
                        span_max = span_max.max(mx);
                    } else {
                        kid_min = kid_min.max(mn);
                        kid_max = kid_max.max(mx);
                    }
                }
                let n = count.max(1) as f32;
                let gap_extra = gap * (n - 1.0);
                let (mut mn, mut mx) = (kid_min, kid_max);
                match col_w.filter(|w| *w > 0.0) {
                    // «The min-content inline size of a multi-column container
                    // with a computed column-width not auto is the smaller of its
                    // column-width and the largest min-content inline-size
                    // contribution of its contents.»
                    Some(w) => {
                        mn = mn.min(w);
                        mx = mx.max(w).max(mn);
                    }
                    // «…with a computed column-width of auto is the largest
                    // min-content inline-size contribution of its contents
                    // multiplied by its column-count …, plus its column-gap
                    // multiplied by column-count minus 1.» При ЗАДАННОЙ ширине
                    // колонки минимум на число колонок не умножается (Blink
                    // :482 — «column-count … is ignored in intrinsic min
                    // inline-size calculation, if column-width is specified»).
                    None => mn = mn * n + gap_extra,
                }
                mx = mx * n + gap_extra;
                (mn.max(span_min), mx.max(span_max))
            });
        let mut baselines = (!axis.is_vertical())
            .then(|| super::column_baselines::Measurements::new(&mut self.children, window, cx));
        let id = window.request_measured_layout_with_baselines(
            gpui::Style::default(),
            move |known, available, window, cx| {
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
                    let probe = ColumnStack {
                        children: Vec::new(),
                        count,
                        gap,
                        axis,
                        row_phase,
                        fixed_height: fixed,
                        rule: None,
                        rule_to: None,
                        rows,
                        copies,
                        gap_items: None,
                        intrinsic: None,
                        plan: std::cell::RefCell::new(Vec::new()),
                        col_w: std::cell::Cell::new(0.0),
                        lines_plan: std::cell::RefCell::new(Vec::new()),
                        spans_plan: std::cell::RefCell::new(Vec::new()),
                    };
                    let (block, _, _, _) = probe.balance(&heights);
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
                let probe = ColumnStack {
                    children: Vec::new(),
                    count,
                    gap,
                    axis,
                    row_phase,
                    fixed_height: fixed,
                    rule: None,
                    rule_to: None,
                    rows,
                    copies,
                    gap_items: None,
                    intrinsic: None,
                    plan: std::cell::RefCell::new(Vec::new()),
                    col_w: std::cell::Cell::new(0.0),
                    lines_plan: std::cell::RefCell::new(Vec::new()),
                    spans_plan: std::cell::RefCell::new(Vec::new()),
                };
                let (height, lines, plan, spans) = probe.balance(&heights);
                let (first, last) = baselines.as_mut().map_or((None, None), |baselines| {
                    baselines.measure(&probe, &heights, w, &lines, &plan, &spans, window, cx)
                });
                (size(px(w), px(height)), first, last)
            },
        );
        (id, id)
    }
}
