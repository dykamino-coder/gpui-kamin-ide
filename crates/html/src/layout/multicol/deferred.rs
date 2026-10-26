//! Стопка при ещё неизвестной ширине: независимые копии для меры и отрисовки.

use crate::dom::{Element, Node};
use crate::layout::fragment::line_shape::side_margin_wrap;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::layout::multicol::measure::laid;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::{AnyElement, App, Pixels, Window};
mod children;
mod geometry;
#[cfg(test)]
mod oof_tests;

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
        let prepared = super::measure::prepared(host, merged, opts, axis.is_vertical(), None);
        if axis.is_vertical() {
            laid::pending_vertical(
                &prepared,
                merged,
                opts.cx.vcx(),
                axis == crate::layout::fragment::types::StackAxis::VerticalRl,
            )?;
        } else {
            laid::pending(&prepared, merged, opts.cx.vcx()).or_else(|| {
                (!opts.frag.in_stack_copy && rows.is_none_or(|r| r.cap))
                    .then(|| {
                        super::measure::native_float::pending(&prepared, merged, opts.cx.vcx())
                    })
                    .flatten()
            })?;
        }
        Some(std::rc::Rc::new(Self {
            host: host.clone(),
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
    pub(crate) fn prepare(
        &self,
        stack: &mut ColumnStack,
        width: f32,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (children, geometry) = self.recorded_children(width, window, cx);
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
