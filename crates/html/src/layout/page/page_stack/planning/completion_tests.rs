//! Неполный запас копий не становится полным планом после сходимости.
use super::settle;
use crate::layout::fragment::plan::{self, Space};
use crate::layout::fragment::types::Kid;

#[test]
fn settlement_preserves_exhausted_copy_status() {
    use crate::layout::fragment::plan::Completion;
    let (_, plan) = settle(|| {
        let (pages, _, plan) = plan::fill(
            &[Kid {
                h: 300.0,
                over: 300.0,
                ..Default::default()
            }],
            100.0,
            Space::pages(1),
        );
        (true, 0, (pages, plan))
    })
    .unwrap();
    assert_eq!(plan.completion, Completion::Incomplete);
    let stack = super::super::PageStack::new(
        Vec::new(),
        std::rc::Rc::new(|_, _| unreachable!()),
        Vec::new(),
        0.0,
        Vec::new(),
        None,
    );
    stack.complete.set(true);
    *stack.plan.borrow_mut() = plan;
    assert!(!stack.has_complete_plan());
}
