//! План clamp и неизменяемый вход сборки; адаптер сохраняет результаты ClampState.

use super::{ClampMeasure, ClampState};
use std::{collections::HashMap, rc::Rc};

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
#[allow(dead_code)] // Вход измерения подключается хостом clamp.
pub(crate) enum ClampInput {
    Measure(ClampMeasure),
    Apply(Rc<ClampPlan>),
}

#[derive(Clone, Default)]
pub(crate) struct ClampPlans {
    plans: HashMap<u64, ClampInput>,
    boxes: HashMap<u64, f32>,
}

impl ClampPlans {
    pub(crate) fn snapshot(state: &ClampState) -> Rc<Self> {
        let cuts = state.cuts.borrow();
        let paras = state.para.borrow();
        let next = state.next.borrow();
        let extent = state.extent.borrow();
        let boxes = state.boxes.borrow().clone();
        // ClampCut публикует полный результат до следующей сборки.
        // Отсутствующий срез отличается от измеренного бесконечного.
        let plans = cuts
            .iter()
            .map(|(key, cut)| {
                (
                    *key,
                    ClampInput::Apply(Rc::new(ClampPlan {
                        nodes: Vec::new(),
                        sizes: Vec::new(),
                        para: paras.get(key).copied(),
                        next: next.get(key).copied(),
                        extent: extent.get(key).copied(),
                        cut: *cut,
                    })),
                )
            })
            .collect();
        Rc::new(Self { plans, boxes })
    }
    pub(crate) fn plan(&self, key: u64) -> Option<&ClampPlan> {
        match self.plans.get(&key)? {
            ClampInput::Apply(plan) => Some(plan),
            ClampInput::Measure(_) => None,
        }
    }
    pub(crate) fn box_height(&self, node: u64) -> Option<f32> {
        self.boxes.get(&node).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::cx::LayoutCx;

    #[test]
    fn snapshot_preserves_missing_and_infinite_cuts_and_survives_publication() {
        let state = ClampState::default();
        state.cuts.borrow_mut().insert(1, f32::INFINITY);
        state.boxes.borrow_mut().insert(9, 42.0);
        let snapshot = ClampPlans::snapshot(&state);
        assert!(snapshot.plan(2).is_none());
        assert_eq!(snapshot.plan(1).unwrap().cut, f32::INFINITY);
        state.cuts.borrow_mut().insert(1, 10.0);
        state.boxes.borrow_mut().clear();
        assert_eq!(snapshot.plan(1).unwrap().cut, f32::INFINITY);
        assert_eq!(snapshot.box_height(9), Some(42.0));
        assert_eq!(ClampPlans::snapshot(&state).plan(1).unwrap().cut, 10.0);
    }

    #[test]
    fn local_host_input_overrides_only_its_own_clamp_root() {
        let state = ClampState::default();
        state.cuts.borrow_mut().extend([(1, 10.0), (2, 20.0)]);
        let mut cx = LayoutCx {
            clamp_plans: ClampPlans::snapshot(&state),
            ..Default::default()
        };
        cx.clamp_build = cx.clamp_build.container(1);
        let plan = Rc::new(ClampPlan {
            nodes: vec![9],
            sizes: vec![(9, 30.0)],
            para: Some((0, 2)),
            next: None,
            extent: None,
            cut: 30.0,
        });
        cx.clamp_input = Some(ClampInput::Apply(plan));
        assert_eq!(cx.clamp_plan(1).unwrap().cut, 30.0);
        assert_eq!(cx.clamp_plan(2).unwrap().cut, 20.0);
        assert_eq!(cx.clamp_box_height(9), Some(30.0));
        cx.clamp_input = Some(ClampInput::Measure(ClampMeasure::default()));
        assert!(cx.clamp_plan(1).is_none());
        assert!(cx.clamp_box_height(9).is_none());
        assert_eq!(cx.clamp_plan(2).unwrap().cut, 20.0);
    }
}
