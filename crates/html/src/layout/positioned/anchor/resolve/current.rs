//! Размеры от опубликованных якорей текущего кадра, до построения корня OOF.

use super::super::AnchorPlan;
use crate::document::state::ValueCx;
use crate::style::computed::{Align, Computed, Position};
use crate::style::values::value::Len;
use gpui::{Bounds, Pixels, Window};

#[derive(Clone, Copy, Default)]
pub(crate) struct SizeInputs {
    pub(super) area: Option<(f32, f32)>,
    pub(super) cell: Option<(f32, f32)>,
}

pub(crate) fn needs_deferred(c: &Computed) -> bool {
    matches!(c.position, Some(Position::Absolute | Position::Fixed))
        && (!c.position_try_fallbacks.is_empty()
            || c.position_area.is_some()
            || c.align_self == Some(Align::AnchorCenter)
            || c.justify_self == Some(Align::AnchorCenter)
            || [
                c.width,
                c.height,
                c.min_width,
                c.min_height,
                c.max_width,
                c.max_height,
                c.inset.top,
                c.inset.right,
                c.inset.bottom,
                c.inset.left,
            ]
            .into_iter()
            .any(|l| matches!(l, Some(Len::Anchor(_)))))
}

pub(crate) fn current_inputs(
    c: &Computed,
    inherited: &Computed,
    bounds: Bounds<Pixels>,
    window: &Window,
    vcx: ValueCx<'_>,
) -> SizeInputs {
    match AnchorPlan::of(c, inherited, vcx) {
        Some(mut plan) => {
            plan.history = false;
            let has_area = plan.area.is_some() && plan.rec_current(None).is_some();
            let p = plan.compute(bounds, window, vcx);
            SizeInputs {
                area: Some(p.imcb),
                cell: has_area.then_some(p.cell),
            }
        }
        None => SizeInputs::default(),
    }
}

pub(crate) fn resolve_current(
    c: &mut Computed,
    inherited: &Computed,
    inputs: SizeInputs,
    vcx: ValueCx<'_>,
) {
    let used = vcx.doc.anchor_frame.used_last.replace(false);
    super::resolve_with(c, inherited, Some(inputs), vcx);
    debug_assert!(
        !vcx.doc.anchor_frame.used_last.get(),
        "deferred OOF read previous-frame sizing"
    );
    vcx.doc.anchor_frame.used_last.set(used);
}

#[cfg(test)]
#[path = "current_tests.rs"]
mod tests;

/// Общая подготовка размеров блочного и строчного OOF в контексте хоста.
pub(crate) fn prepare(c: &mut Computed, inherited: &Computed, cx: &crate::layout::cx::LayoutCx) {
    let vcx = cx.vcx();
    if cx
        .oof_trial
        .as_ref()
        .is_none_or(|(key, _)| *key != c.anchor_key)
    {
        super::apply_chosen(c, vcx);
    }
    if let Some((key, inputs)) = cx.oof_sizes
        && key == c.anchor_key
    {
        resolve_current(c, inherited, inputs, vcx);
    } else {
        super::resolve_sizes(c, inherited, vcx);
    }
}
