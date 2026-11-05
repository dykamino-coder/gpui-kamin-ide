//! Стопка при ещё неизвестной ширине: независимые копии для меры и отрисовки.

use crate::dom::{Element, Node};
use crate::layout::fragment::line_shape::side_margin_wrap;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::layout::multicol::measure::laid;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::{AnyElement, App, Pixels, Window};
#[cfg(test)]
mod balance_tests;
mod children;
#[cfg(test)]
mod clip_tests;
mod geometry;
#[cfg(test)]
mod oof_tests;
#[cfg(test)]
mod table_row_tests;

/// Исходное дерево, без LayoutId и изменяемых рекордеров от пробной раскладки.
pub(crate) struct Recipe {
    pub(crate) host: Element,
    pub(crate) merged: BoxStyle,
    pub(crate) opts: RenderOpts,
    pub(crate) count: u16,
    pub(crate) fixed: Option<f32>,
    pub(crate) axis: crate::layout::fragment::types::StackAxis,
    pub(crate) rows: Option<crate::layout::fragment::types::Rows>,
    /// Прямые позиционированные дети с местом в потоке и их щупы.
    #[allow(clippy::type_complexity)]
    pub(crate) statics: std::cell::RefCell<(
        Vec<(usize, Element)>,
        Vec<crate::layout::positioned::containing_block::SpotCell>,
    )>,
    /// Builds of the children measured for the stack's intrinsic size.
    #[cfg(test)]
    pub(crate) intrinsic_builds: std::cell::Cell<usize>,
    /// The row extents of the spanner flow this stack is a row of.
    row: Option<std::rc::Rc<crate::layout::multicol::spanner::RowSlot>>,
}

impl Recipe {
    // Все независимые ограничения колонки сохраняются до известной ширины.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        host: &Element,
        merged: &BoxStyle,
        opts: &RenderOpts,
        count: u16,
        fixed: Option<f32>,
        axis: crate::layout::fragment::types::StackAxis,
        rows: Option<crate::layout::fragment::types::Rows>,
    ) -> Option<std::rc::Rc<Self>> {
        let prepared = super::measure::prepared(host, merged, opts, axis.is_vertical(), None);
        // Каждый поточный ребёнок получает запись: при известной ширине его меру
        // даёт раскладка копии (`plan_at`).
        laid::pending_mixed(&prepared, merged, opts.cx.frame())?;
        let mut opts = opts.clone();
        let row = (opts.cx.fragmentation.spanner_rows.take()).filter(|s| s.is_row_of(host.node_id));
        Some(std::rc::Rc::new(Self {
            host: host.clone(),
            merged: merged.clone(),
            opts,
            count,
            fixed,
            axis,
            rows,
            statics: Default::default(),
            #[cfg(test)]
            intrinsic_builds: Default::default(),
            row,
        }))
    }

    pub(crate) fn intrinsic_children(&self) -> Vec<AnyElement> {
        #[cfg(test)]
        self.intrinsic_builds.set(self.intrinsic_builds.get() + 1);
        self.host
            .children
            .iter()
            .filter_map(|n| {
                let Node::Element(c) = n else { return None };
                Some(side_margin_wrap(
                    self.opts.cx.contracts.build.element(
                        c,
                        &self.merged,
                        &self.opts.in_stack_copy(),
                    ),
                    c,
                    false,
                ))
            })
            .collect()
    }

    /// После определения ширины используется тот же CB, что в stack_plan.
    fn resolved_host(&self, width: f32) -> Element {
        super::measure::prepared(
            &self.host,
            &self.merged,
            &self.opts,
            self.axis.is_vertical(),
            Some(width),
        )
    }

    /// Один и тот же проход до баланса в измерительном callback и prepaint.
    pub(crate) async fn prepare_async(
        &self,
        stack: &mut ColumnStack,
        width: f32,
        window: &mut Window,
        cx: &mut App,
        scheduler: &mut gpui::PaintStack,
    ) {
        stack.forced_h = None;
        stack.balance_failure = None;
        stack.col_w.set(width);
        let (children, geometry) = self
            .recorded_children_async(width, window, cx, scheduler)
            .await;
        stack.children = children;
        stack.oof.geometry = geometry;
        if stack.fixed_height.is_some() {
            stack.copies = stack
                .children
                .iter()
                .map(|c| c.frags.len() + 1)
                .max()
                .unwrap_or(1)
                .max(stack.count);
        }
        for i in 0..stack.children.len() {
            if !stack.resume_nested_async(i, window, cx, scheduler).await {
                stack.lay_out_main_async(i, window, cx, scheduler).await;
            }
        }
        stack.regrow_children_async(window, cx, scheduler).await;
        stack.paginate_children_async(window, cx, scheduler).await;
        stack.reserve_overflow_copies();
        stack.report_lead();
    }

    pub(crate) async fn measure_async(
        &self,
        stack: &mut ColumnStack,
        width: f32,
        window: &mut Window,
        cx: &mut App,
        scheduler: &mut gpui::PaintStack,
    ) -> (f32, Option<Pixels>, Option<Pixels>) {
        if let Some(extent) = self.row.as_ref().and_then(|row| row.get(width)) {
            crate::work::add!(SpannerRowReuses, 1);
            return extent;
        }
        let extent = gpui::with_temporary_element_arena_async(async || {
            window
                .with_retained_layout_async(&mut gpui::RetainedLayout::default(), async |window| {
                    let col_width = ((width - stack.gap * (stack.count as f32 - 1.0))
                        / stack.count as f32)
                        .max(1.0);
                    self.prepare_async(stack, col_width, window, cx, scheduler)
                        .await;
                    self.measured_extent_async(stack, width, window, cx, scheduler)
                        .await
                })
                .await
        })
        .await;
        if let Some(row) = &self.row {
            row.put(width, extent);
        }
        extent
    }

    /// Высота и базовые линии подготовленной стопки — отдельным кадром после
    /// вложенных раскладок `prepare`.
    #[inline(never)]
    async fn measured_extent_async(
        &self,
        stack: &mut ColumnStack,
        width: f32,
        window: &mut Window,
        cx: &mut App,
        scheduler: &mut gpui::PaintStack,
    ) -> (f32, Option<Pixels>, Option<Pixels>) {
        let kids = stack.kid_geoms();
        let (height, lines, plan, spans) = stack.balance(&kids);
        let height = stack
            .row_extent
            .block_size(height, &lines, &spans, &plan, stack.count, &kids);
        let (first, last) = if self.axis.is_vertical() {
            (None, None)
        } else {
            let mut baselines = super::column_baselines::Measurements::new_async(
                &mut stack.children,
                window,
                cx,
                scheduler,
            )
            .await;
            baselines
                .measure_async(
                    stack, &kids, width, &lines, &plan, &spans, window, cx, scheduler,
                )
                .await
        };
        // Элементы с LayoutId вложенного движка не переживают этот вызов.
        stack.children.clear();
        stack.oof.geometry = Default::default();
        (height, first, last)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
impl Recipe {
    pub(crate) fn prepare(
        &self,
        column: &mut ColumnStack,
        width: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> () {
        reblessive::Stack::new()
            .enter(|stack| self.prepare_async(column, width, window, cx, stack))
            .finish()
    }
}

#[cfg(test)]
impl Recipe {
    pub(crate) fn measure(
        &self,
        column: &mut ColumnStack,
        width: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> (f32, Option<Pixels>, Option<Pixels>) {
        reblessive::Stack::new()
            .enter(|stack| self.measure_async(column, width, window, cx, stack))
            .finish()
    }
}
