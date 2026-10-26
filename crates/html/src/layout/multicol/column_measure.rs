//! Запрос раскладки стопки колонок до отрисовки: замер высоты каждого ребёнка при ширине
//! колонки, разбиение детей на страницы, внутренние размеры многоколоночника по
//! css-sizing-4 §multicol-intrinsic (как Blink `ComputeMinMaxSizes`) для случаев, когда
//! ширину решает содержимое, и пробная балансировка, дающая размер коробки и базовые линии.

use crate::layout::fragment::plan::Plan;
use crate::layout::fragment::types::{BreaksAt, Intrinsic};
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{AnyElement, App, LayoutId, Window, px, size};

impl ColumnStack {
    pub(crate) fn request_column_layout(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        if self.deferred.is_none() {
            for i in 0..self.children.len() {
                self.lay_out_main(i, window, cx);
            }
            self.regrow_children(window, cx);
            self.paginate_children(window, cx);
        }
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
        let mut intrinsic_children = self
            .deferred
            .as_ref()
            .map(|r| r.intrinsic_children())
            .unwrap_or_default();
        let deferred_intrinsic = self.deferred.is_some();
        let intrinsic = self
            .intrinsic
            .filter(|_| !axis.is_vertical())
            .map(|Intrinsic(col_w)| {
                let (mut kid_min, mut kid_max) = (0.0f32, 0.0f32);
                let (mut span_min, mut span_max) = (0.0f32, 0.0f32);
                for (el, span) in self
                    .children
                    .iter_mut()
                    .filter(|_| !deferred_intrinsic)
                    .map(|c| (&mut c.el, c.span))
                    .chain(intrinsic_children.iter_mut().map(|el| (el, false)))
                {
                    let mut measure = |el: &mut AnyElement, w: gpui::AvailableSpace| {
                        f32::from(
                            el.layout_as_root_unrounded(
                                size(w, gpui::AvailableSpace::MaxContent),
                                window,
                                cx,
                            )
                            .width,
                        )
                    };
                    let mn = measure(el, gpui::AvailableSpace::MinContent);
                    let mx = measure(el, gpui::AvailableSpace::MaxContent);
                    // Спаннер идёт во всю ширину коробки: на число колонок он не
                    // умножается, а лишь ПОДНИМАЕТ итог — Blink
                    // `ComputeSpannersMinMaxSizes` (:523) через
                    // `MinMaxSizes::Encompass` (`min_max_sizes.h:26`, это `max`
                    // по обеим границам).
                    if span {
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
        let mut baselines = (!axis.is_vertical() && self.deferred.is_none()).then(|| {
            crate::layout::multicol::column_baselines::Measurements::new(
                &mut self.children,
                window,
                cx,
            )
        });
        let paint = self.paint.clone();
        let deferred = self.deferred.clone();
        let id = window.request_measured_layout_with_baselines(
            gpui::Style::default(),
            move |known, available, window, cx| {
                let mut probe = ColumnStack {
                    paint: paint.clone(),
                    children: Vec::new(),
                    deferred: None,
                    count,
                    gap,
                    axis,
                    row_phase,
                    row_extent,
                    fixed_height: fixed,
                    forced_h,
                    balance_failure,
                    fill_shrink,
                    rule: None,
                    rule_to: None,
                    rows,
                    copies,
                    gap_items: None,
                    intrinsic: None,
                    plan: std::cell::RefCell::new(Plan::default()),
                    static_places: Default::default(),
                    oof: oof.fresh(),
                    col_w: std::cell::Cell::new(0.0),
                    lines_plan: std::cell::RefCell::new(Vec::new()),
                    spans_plan: std::cell::RefCell::new(Vec::new()),
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
                        recipe.measure(&mut probe, inline, window, cx).0
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
                    let (height, first, last) = recipe.measure(&mut probe, w, window, cx);
                    return (size(px(w), px(height)), first, last);
                }
                probe
                    .col_w
                    .set(((w - gap * (count as f32 - 1.)) / count as f32).max(1.));
                let (height, lines, plan, spans) = probe.balance(&heights);
                let height = row_extent.block_size(height, &lines, &spans, &plan, count, &heights);
                let (first, last) = baselines.as_mut().map_or((None, None), |baselines| {
                    baselines.measure(&probe, &heights, w, &lines, &plan, &spans, window, cx)
                });
                (size(px(w), px(height)), first, last)
            },
        );
        (id, id)
    }

    /// Измеренные точки разрыва детей, чья колонка до раскладки коробки
    /// стопки неизвестна (`BreaksAt::Column`): главная копия раскладывается
    /// шириной `col_w`, и её строки и блочная стопка заменяют оценку. Только
    /// при высоте колонки, заданной до меры детей (`fixed_height`): высоту
    /// балансируемой коробки решила мера при запросе раскладки.
    pub(crate) fn column_breaks(&mut self, col_w: f32, window: &mut Window, cx: &mut App) {
        if self.fixed_height.is_none() {
            return;
        }
        for c in self.children.iter_mut() {
            if c.measure.is_some() || c.span || c.breaks_w != Some(BreaksAt::Column) {
                continue;
            }
            let sz = c.el.layout_as_root(
                gpui::size(
                    gpui::AvailableSpace::Definite(px(col_w)),
                    gpui::AvailableSpace::MaxContent,
                ),
                window,
                cx,
            );
            #[cfg(feature = "wpt-harness")]
            if c.probe_pending && c.watched.is_some() {
                c.probe.height(
                    "columns.column",
                    c.h,
                    f32::from(if self.axis.is_vertical() {
                        sz.width
                    } else {
                        sz.height
                    }),
                );
            }
            c.replace_breaks(sz, self.axis, window);
        }
    }
}
