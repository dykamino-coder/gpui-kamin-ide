//! Регистрация проверок нижних данных под публично устоявшимися именами.

mod tests {
    #[test]
    fn snapshots_share_storage_and_survive_later_registrations() {
        crate::values::property_registry::inventory_cases::snapshots_share_storage_and_survive_later_registrations();
    }
}
