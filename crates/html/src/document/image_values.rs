//! Регистрация проверок нижних данных под публично устоявшимися именами.

mod tests {
    #[test]
    fn pending_operand_survives_style_until_preparation_and_old_view_stays_pending() {
        crate::values::image_values::inventory_cases::pending_operand_survives_style_until_preparation_and_old_view_stays_pending();
    }
    #[test]
    fn prepare_visits_layers_pseudo_styles_animation_and_nested_operands() {
        crate::values::image_values::inventory_cases::prepare_visits_layers_pseudo_styles_animation_and_nested_operands();
    }
    #[test]
    fn frame_refresh_publishes_changed_pixels_without_mutating_a_held_view() {
        crate::values::image_values::inventory_cases::frame_refresh_publishes_changed_pixels_without_mutating_a_held_view();
    }
}
