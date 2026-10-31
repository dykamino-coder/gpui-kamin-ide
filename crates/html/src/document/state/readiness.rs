//! Регистрация проверок нижних данных под публично устоявшимися именами.

mod tests {
    #[test]
    fn nested_terminal_failure_is_visible_to_the_root_document() {
        crate::values::readiness::inventory_cases::nested_terminal_failure_is_visible_to_the_root_document();
    }
    #[test]
    fn nested_traversal_limit_reaches_the_root_without_rejecting_it() {
        crate::values::readiness::inventory_cases::nested_traversal_limit_reaches_the_root_without_rejecting_it();
    }
    #[test]
    fn paint_and_all_nested_work_must_finish() {
        crate::values::readiness::inventory_cases::paint_and_all_nested_work_must_finish();
    }
}
