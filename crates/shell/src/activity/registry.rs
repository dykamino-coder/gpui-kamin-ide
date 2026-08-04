//! Реестр активностей: поиск по id, интернирование, singleton-проверка.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::activity::ActivityItem;
use crate::activity::BUILTIN_ACTIVITIES;
use crate::activity::dyn_tools::{DynTool, dyn_has};

/// Поиск тула в реестре.
pub fn lookup(id: &str) -> Option<&'static ActivityItem> {
    BUILTIN_ACTIVITIES.iter().find(|a| a.id == id)
}
pub fn is_singleton(id: &str) -> bool {
    dyn_has(id)
}
pub fn dyn_tool(id: &str) -> Option<DynTool> {
    crate::activity::dyn_tools::dyn_tools()
        .lock()
        .unwrap()
        .iter()
        .find(|t| t.id == id)
        .cloned()
}
/// (label, icon) тула: builtin ИЛИ contributed.
/// `&'static str` для произвольного id тула. `ShellEvent::ActivityClicked` и
/// `RootView::sidebar_activity` требуют `'static`, а contributed-тулы приходят
/// строками; набор id мал и стабилен — интернируем с однократной утечкой
/// (тот же приём, что у `static_view_id` в `root.rs`).
pub fn static_id(id: &str) -> &'static str {
    if let Some(b) = lookup(id) {
        return b.id;
    }
    intern(id)
}
/// Интернер строк с однократной утечкой (id и имена иконок contributed-тулов).
pub fn intern(s: &str) -> &'static str {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, &'static str>>,
    > = std::sync::OnceLock::new();
    let m = CACHE.get_or_init(Default::default);
    let mut g = m.lock().unwrap();
    if let Some(v) = g.get(s) {
        return v;
    }
    let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
    g.insert(s.to_string(), leaked);
    leaked
}
pub fn lookup_any(id: &str) -> Option<(String, String)> {
    if let Some(b) = lookup(id) {
        return Some((b.label.to_string(), b.icon.to_string()));
    }
    dyn_tool(id).map(|t| (t.label, t.icon))
}
