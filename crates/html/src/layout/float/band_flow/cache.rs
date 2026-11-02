//! Ограниченный кэш планов по точной ширине для layout и prepaint.

use super::Plan;
use std::cell::Cell;
use std::rc::Rc;

const CAPACITY: usize = 4;

#[derive(Default)]
pub(super) struct Plans {
    slots: [Cell<Option<Rc<Plan>>>; CAPACITY],
    next: Cell<usize>,
}

impl Plans {
    pub(super) fn get(&self, width: f32) -> Option<Rc<Plan>> {
        for slot in &self.slots {
            let plan = slot.take();
            let found = plan.as_ref().filter(|p| p.width == width).cloned();
            // Между take и set нет вызова пользовательского кода.
            slot.set(plan);
            if found.is_some() {
                return found;
            }
        }
        None
    }

    pub(super) fn insert(&self, plan: Plan) -> Rc<Plan> {
        if let Some(cached) = self.get(plan.width) {
            return cached;
        }
        // FIFO ограничивает удержание больших подготовленных деревьев независимо
        // от числа запросов ширины. Вытесненный план живёт у текущего потребителя.
        let plan = Rc::new(plan);
        let next = self.next.get();
        self.slots[next].set(Some(plan.clone()));
        self.next.set((next + 1) % CAPACITY);
        plan
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
        let duplicate = prepaint.insert(Plan {
            width: 100.,
            height: 99.,
            ..Default::default()
        });
        assert!(Rc::ptr_eq(&first, &duplicate));
        assert_eq!(prepaint.get(100.).unwrap().height, 12.);
    }

    #[test]
    fn distinct_widths_evict_old_entries_without_invalidating_a_live_plan() {
        let cache = Plans::default();
        let live = cache.insert(Plan {
            width: 0.,
            height: 12.,
            ..Default::default()
        });
        let old = Rc::downgrade(&cache.insert(Plan {
            width: 1.,
            ..Default::default()
        }));
        for width in 2..100 {
            cache.insert(Plan {
                width: width as f32,
                ..Default::default()
            });
        }
        assert!(cache.get(0.).is_none());
        assert!(old.upgrade().is_none());
        assert_eq!(live.height, 12.);
        assert_eq!(Rc::strong_count(&live), 1);
        let retained = (0..100).filter(|w| cache.get(*w as f32).is_some()).count();
        assert_eq!(retained, CAPACITY);
        for width in (100 - CAPACITY)..100 {
            assert!(cache.get(width as f32).is_some());
        }
    }
}
