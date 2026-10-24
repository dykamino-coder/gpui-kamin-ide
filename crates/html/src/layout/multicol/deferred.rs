//! Стопка при ещё неизвестной ширине: независимые копии для меры и отрисовки.

use crate::dom::{Element, Node};
use crate::layout::fragment::line_shape::side_margin_wrap;
use crate::layout::fragment::types::StackChild;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::layout::multicol::container::kids::{column_kids, stack_children};
use crate::layout::multicol::measure::laid;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::{AnyElement, App, Pixels, Window};
use std::cell::RefCell;

/// Исходное дерево, без LayoutId и изменяемых рекордеров от пробной раскладки.
pub(crate) struct Recipe {
    pub(crate) host: Element,
    pub(crate) merged: BoxStyle,
    pub(crate) opts: RenderOpts,
    pub(crate) count: u16,
    pub(crate) gap: f32,
    pub(crate) fixed: Option<f32>,
    pub(crate) axis: crate::layout::fragment::types::StackAxis,
    pub(crate) rows: Option<crate::layout::fragment::types::Rows>,
}

impl Recipe {
    // Все независимые ограничения колонки сохраняются до известной ширины.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        host: &Element,
        merged: &BoxStyle,
        opts: &RenderOpts,
        count: u16,
        gap: f32,
        fixed: Option<f32>,
        axis: crate::layout::fragment::types::StackAxis,
        rows: Option<crate::layout::fragment::types::Rows>,
    ) -> Option<std::rc::Rc<Self>> {
        let host = crate::layout::fragment::line_shape::group_inline_runs(host)
            .unwrap_or_else(|| host.clone());
        if axis.is_vertical() {
            laid::pending_vertical(
                &host,
                merged,
                opts.cx.vcx(),
                axis == crate::layout::fragment::types::StackAxis::VerticalRl,
            )?;
        } else {
            laid::pending(&host, merged, opts.cx.vcx()).or_else(|| {
                (!opts.frag.in_stack_copy && rows.is_none_or(|r| r.cap))
                    .then(|| super::measure::native_float::pending(&host, merged, opts.cx.vcx()))
                    .flatten()
            })?;
        }
        Some(std::rc::Rc::new(Self {
            host,
            merged: merged.clone(),
            opts: opts.clone(),
            count,
            gap,
            fixed,
            axis,
            rows,
        }))
    }

    pub(crate) fn intrinsic_children(&self) -> Vec<AnyElement> {
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

    pub(super) fn children(&self, width: f32) -> Vec<StackChild> {
        let measured = RefCell::default();
        let opts = self.opts.clone();
        let vcx = opts.cx.vcx();
        let vertical = self.axis.is_vertical();
        let reverse = self.axis == crate::layout::fragment::types::StackAxis::VerticalRl;
        let host = self.resolved_host(width);
        let kids = if vertical {
            laid::vertical_children(&host, &self.merged, vcx, width, reverse, &measured)
        } else {
            laid::children(&host, &self.merged, vcx, width, &measured).or_else(|| {
                super::measure::native_float::children(&host, &self.merged, vcx, width, &measured)
            })
        };
        let kids = kids.expect("deferred recipe preserves the measured-tree contract");
        let copies = self.count.max(1) as usize;
        let (parents, _, parallel, kids) = column_kids(
            &self.merged,
            kids,
            self.count,
            self.gap,
            vertical,
            self.rows,
            copies,
            self.fixed,
            (Some(width), Some(width)),
            vcx,
        );
        let prepared = kids
            .iter()
            .enumerate()
            .map(|(ix, (c, s))| {
                super::stack_child::prepare(
                    ix,
                    c,
                    s,
                    parents[ix].as_ref().unwrap_or(&self.merged),
                    &opts,
                    &parallel,
                    vertical,
                    reverse,
                    Some(width),
                    self.fixed,
                    &measured,
                )
            })
            .collect();
        stack_children(
            &host,
            &self.merged,
            &opts,
            self.count,
            vertical,
            reverse,
            (Some(width), Some(width)),
            self.rows,
            RefCell::default(),
            RefCell::default(),
            measured,
            copies,
            self.fixed,
            parents,
            parallel,
            kids,
            Vec::new(),
            prepared,
        )
    }

    /// После определения ширины используется тот же CB, что в stack_plan.
    fn resolved_host(&self, width: f32) -> Element {
        let mut host = self.host.clone();
        if !self.axis.is_vertical() {
            host.children = crate::layout::fragment::flow::column_percentages(
                &host.children,
                &self.merged,
                Some(width),
                &self.opts,
            );
        }
        host
    }

    /// Один и тот же проход до баланса в измерительном callback и prepaint.
    pub(crate) fn prepare(
        &self,
        stack: &mut ColumnStack,
        width: f32,
        window: &mut Window,
        cx: &mut App,
    ) {
        stack.children = self.children(width);
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
            stack.lay_out_main(i, window, cx);
        }
        stack.regrow_children(window, cx);
        stack.paginate_children(window, cx);
    }

    pub(crate) fn measure(
        &self,
        stack: &mut ColumnStack,
        width: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> (f32, Option<Pixels>, Option<Pixels>) {
        window.with_nested_layout(|window| {
            let col_width =
                ((width - stack.gap * (stack.count as f32 - 1.0)) / stack.count as f32).max(1.0);
            self.prepare(stack, col_width, window, cx);
            let kids = stack.kid_geoms();
            let (height, lines, plan, spans) = stack.balance(&kids);
            let (first, last) = if self.axis.is_vertical() {
                (None, None)
            } else {
                let mut baselines =
                    super::column_baselines::Measurements::new(&mut stack.children, window, cx);
                baselines.measure(stack, &kids, width, &lines, &plan, &spans, window, cx)
            };
            // Элементы с LayoutId вложенного движка не переживают этот вызов.
            stack.children.clear();
            (height, first, last)
        })
    }
}

#[cfg(test)]
mod tests;
