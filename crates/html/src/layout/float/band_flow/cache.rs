//! Планы разных ограничений не вытесняют друг друга при измерении одного элемента.

use super::Plan;
use std::rc::Rc;

#[derive(Default)]
pub(super) struct Plans(Vec<Rc<Plan>>);

impl Plans {
    pub(super) fn get(&self, width: f32) -> Option<Rc<Plan>> {
        self.0.iter().find(|plan| plan.width == width).cloned()
    }

    pub(super) fn insert(&mut self, plan: Plan) -> Rc<Plan> {
        let plan = Rc::new(plan);
        self.0.push(plan.clone());
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternate_widths_keep_the_original_plan_and_fractional_edges() {
        let mut cache = Plans::default();
        let first = cache.insert(Plan {
            width: 100.0,
            height: 12.0,
            ..Default::default()
        });
        cache.insert(Plan {
            width: 200.0,
            height: 6.0,
            ..Default::default()
        });
        assert!(Rc::ptr_eq(&first, &cache.get(100.0).unwrap()));
        assert_eq!(cache.get(200.0).unwrap().height, 6.0);
        assert!(cache.get(100.000_01).is_none());
    }
}
