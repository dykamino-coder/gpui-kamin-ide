//! Жёсткий бюджет нативных повторов для тестов сходимости, без часов и таймеров.
use std::cell::RefCell;

thread_local! {
    static BUDGET: RefCell<Option<(usize, usize, Vec<String>)>> = const { RefCell::new(None) };
}

/// Считает повторы layout в текущем тестовом потоке; превышение — assertion.
/// Для наблюдения нужен обычный `Trace`, как и для журнала документа.
pub struct LayoutPassBudget;

impl LayoutPassBudget {
    /// Начать независимый счётчик повторов текущего теста.
    pub fn new(limit: usize) -> Self {
        BUDGET.with(|budget| {
            let mut budget = budget.borrow_mut();
            assert!(budget.is_none(), "nested layout pass budget");
            *budget = Some((limit, 0, Vec::new()));
        });
        Self
    }

    /// Число нативных повторов, включая вложенные измерительные проходы.
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
