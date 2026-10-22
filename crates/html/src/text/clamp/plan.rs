//! План clamp и неизменяемый вход локальной сборки текущего кадра.

use super::ClampMeasure;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ClampPlan {
    pub nodes: Vec<u64>,
    pub sizes: Vec<(u64, f32)>,
    pub para: Option<(u32, usize)>,
    pub next: Option<u32>,
    pub extent: Option<f32>,
    pub cut: f32,
}

#[derive(Clone)]
pub(crate) enum ClampInput {
    Measure(ClampMeasure),
    Apply(Rc<ClampPlan>),
    ApplyMeasured(Rc<ClampPlan>, ClampMeasure),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::cx::LayoutCx;

    #[test]
    fn plans_are_local_to_the_active_container_and_apply_build() {
        let mut cx = LayoutCx::default();
        cx.clamp_build = cx.clamp_build.container(1);
        let plan = Rc::new(ClampPlan {
            nodes: vec![9],
            sizes: vec![(9, 30.0)],
            para: Some((0, 2)),
            next: None,
            extent: None,
            cut: 30.0,
        });
        for input in [
            ClampInput::Apply(plan.clone()),
            ClampInput::ApplyMeasured(plan.clone(), ClampMeasure::default()),
        ] {
            cx.clamp_input = Some(input);
            assert_eq!(cx.clamp_plan(1), Some(plan.as_ref()));
            assert!(cx.clamp_plan(2).is_none());
            assert_eq!(cx.clamp_box_height(9), Some(30.0));
            assert!(cx.clamp_box_height(10).is_none());
        }
        cx.clamp_input = Some(ClampInput::Measure(ClampMeasure::default()));
        assert!(cx.clamp_plan(1).is_none());
        assert!(cx.clamp_box_height(9).is_none());
        cx.clamp_input = None;
        assert!(cx.clamp_plan(1).is_none());
        assert!(cx.clamp_plan(2).is_none());
        assert!(cx.clamp_box_height(9).is_none());
    }
}
