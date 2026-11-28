//! A deterministic native retry budget for layout convergence tests.
use std::cell::RefCell;

thread_local! {
    static BUDGET: RefCell<Option<(usize, usize, Vec<String>)>> = const { RefCell::new(None) };
}

/// Counts native layout retries on this test thread and asserts the budget.
/// Requires an active `Trace`, like the document journal.
pub struct LayoutPassBudget;

impl LayoutPassBudget {
    /// Starts an independent retry counter for the current test.
    pub fn new(limit: usize) -> Self {
        BUDGET.with(|budget| {
            let mut budget = budget.borrow_mut();
            assert!(budget.is_none(), "nested layout pass budget");
            *budget = Some((limit, 0, Vec::new()));
        });
        Self
    }

    /// Returns native retries, including nested measurement passes.
    pub fn retries(&self) -> usize {
        BUDGET.with(|budget| budget.borrow().as_ref().unwrap().1)
    }
}

impl Drop for LayoutPassBudget {
    fn drop(&mut self) {
        BUDGET.with(|budget| *budget.borrow_mut() = None);
    }
}

pub(super) fn observe(kind: &str, detail: &str) {
    if kind != "layout.dependency_retry" {
        return;
    }
    BUDGET.with(|budget| {
        let mut budget = budget.borrow_mut();
        let Some((limit, count, recent)) = budget.as_mut() else { return };
        *count += 1;
        if recent.len() == 12 {
            recent.remove(0);
        }
        recent.push(detail.to_owned());
        assert!(*count <= *limit, "native layout exceeded {limit} retries:\n{}", recent.join("\n"));
    });
}
