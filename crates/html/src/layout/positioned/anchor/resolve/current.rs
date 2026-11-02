//! Размеры от опубликованных якорей текущего кадра, до построения корня OOF.

use super::super::AnchorPlan;
use crate::style::computed::{Align, BoxStyle, Computed, Position};
use crate::style::values::value::Len;
use crate::values::ValueCx;
use gpui::{Bounds, Pixels, Window};

#[derive(Clone, Copy, Default)]
pub(crate) struct SizeInputs {
    pub(super) area: Option<(f32, f32)>,
    pub(super) cell: Option<(f32, f32)>,
    pub(crate) refs_default: bool,
}

pub(crate) fn needs_deferred(c: &Computed) -> bool {
    matches!(
        c.reset_positioned.position,
        Some(Position::Absolute | Position::Fixed)
    ) && (!c.reset_positioned.position_try_fallbacks.is_empty()
        || c.reset_positioned.position_visibility
            & (super::super::parse::VIS_VALID | super::super::parse::VIS_VISIBLE)
            != 0
        || c.reset_positioned.position_area.is_some()
        || c.reset_flex.align_self == Some(Align::AnchorCenter)
        || c.reset_flex.justify_self == Some(Align::AnchorCenter)
        || [
            c.reset_box_layout.width,
            c.reset_box_layout.height,
            c.reset_box_layout.min_width,
            c.reset_box_layout.min_height,
            c.reset_box_layout.max_width,
            c.reset_box_layout.max_height,
            c.reset_positioned.inset.top,
            c.reset_positioned.inset.right,
            c.reset_positioned.inset.bottom,
            c.reset_positioned.inset.left,
        ]
        .into_iter()
        .any(|l| matches!(l, Some(Len::Anchor(_)))))
}

/// Потомки построенного на prepaint OOF используют тот же протокол CB:
/// нулевая строчная обёртка не может разрешать их процентные размеры.
pub(crate) fn needs_host(c: &Computed, cx: &crate::layout::cx::LayoutCx) -> bool {
    needs_deferred(c)
        || (cx.position.oof_sizes.is_some()
            && matches!(
                c.reset_positioned.position,
                Some(Position::Absolute | Position::Fixed)
            ))
}

pub(crate) fn current_inputs(
    c: &BoxStyle,
    inherited: &BoxStyle,
    bounds: Bounds<Pixels>,
    window: &Window,
    vcx: ValueCx<'_>,
    layout: &std::rc::Rc<crate::layout::layout_state::LayoutState>,
) -> SizeInputs {
    match AnchorPlan::of(c, inherited, vcx, layout) {
        Some(plan) => {
            let has_area = plan.area.is_some() && plan.rec_current(None).is_some();
            let p = plan.compute(bounds, window, vcx);
            SizeInputs {
                area: Some(p.imcb),
                cell: has_area.then_some(p.cell),
                refs_default: plan.refs_default,
            }
        }
        None => SizeInputs::default(),
    }
}

pub(crate) fn resolve_current(
    c: &mut BoxStyle,
    inherited: &BoxStyle,
    inputs: SizeInputs,
    vcx: ValueCx<'_>,
    layout: &std::rc::Rc<crate::layout::layout_state::LayoutState>,
) {
    super::resolve_with(c, inherited, Some(inputs), vcx, layout);
}

#[cfg(test)]
#[path = "current_tests.rs"]
mod tests;

/// Общая подготовка размеров блочного и строчного OOF в контексте хоста.
pub(crate) fn prepare(c: &mut BoxStyle, inherited: &BoxStyle, cx: &crate::layout::cx::LayoutCx) {
    let vcx = cx.vcx();
    if let Some((key, computed)) = &cx.position.oof_computed
        && *key == c.anchor_key
    {
        c.element_style = Some(computed.clone());
    }
    if let Some((key, inputs)) = cx.position.oof_sizes
        && key == c.anchor_key
    {
        resolve_current(c, inherited, inputs, vcx, cx.layout());
    } else {
        super::resolve_sizes(c, inherited, vcx, cx.layout());
    }
}
