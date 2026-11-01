//! Планы разных ограничений не вытесняют друг друга при измерении одного элемента.

use super::Plan;
use std::cell::OnceCell;
use std::rc::Rc;

#[derive(Default)]
pub(super) struct Plans(OnceCell<Entry>);

struct Entry {
    plan: Rc<Plan>,
    next: Box<Plans>,
}

impl Plans {
    pub(super) fn get(&self, width: f32) -> Option<Rc<Plan>> {
        let mut slot = self;
        while let Some(entry) = slot.0.get() {
            if entry.plan.width == width {
                return Some(entry.plan.clone());
            }
            slot = &entry.next;
        }
        None
    }

    pub(super) fn insert(&self, plan: Plan) -> Rc<Plan> {
        // План публикуется один раз и больше не меняется. Layout и prepaint
        // разделяют готовые записи; новая ширина занимает собственный слот.
        let mut slot = self;
        while let Some(entry) = slot.0.get() {
            if entry.plan.width == plan.width {
                return entry.plan.clone();
            }
            slot = &entry.next;
        }
        slot.0
            .get_or_init(|| Entry {
                plan: Rc::new(plan),
                next: Box::default(),
            })
            .plan
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternate_widths_keep_the_original_plan_and_fractional_edges() {
        let cache = Plans::default();
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

    #[test]
    fn publishing_a_width_twice_preserves_the_prepared_plan() {
        let cache = Rc::new(Plans::default());
        let prepaint = cache.clone();
        let first = cache.insert(Plan {
            width: 100.,
            height: 12.,
            ..Default::default()
        });
        for width in 101..120 {
            cache.insert(Plan {
                width: width as f32,
                ..Default::default()
            });
        }
        let duplicate = prepaint.insert(Plan {
            width: 100.,
            height: 99.,
            ..Default::default()
        });
        assert!(Rc::ptr_eq(&first, &duplicate));
        assert_eq!(prepaint.get(100.).unwrap().height, 12.);
        assert!(prepaint.get(119.).is_some());
    }
}
