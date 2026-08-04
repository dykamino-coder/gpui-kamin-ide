//! Поля `RootView`, вынесенные группой `cz`
//! (`plan/100-refactor-250.md`).

use gpui::Entity;
use gpui_component::input::InputState;

#[derive(Default)]
pub struct CustomizeState {
    /// Свёрнут ли узел contributed-страниц Customize.
    /// Свёрнутые contributed-контейнеры Customize (по id).
    pub customize_contrib_collapsed: std::collections::HashSet<String>,
    /// Фильтр-инпут панелей Logs/System (Customize).
    pub log_filter_input: Option<Entity<InputState>>,
    /// Семпл-инпут Design-страницы: у оригинала это живой `<input>` с
    /// `:focus`-рамкой (`component-samples.tsx:88-94`), а не статичный div.
    pub design_input: Option<Entity<InputState>>,
    /// Скролл тела Logs — держит «прилипание» к низу при доливе строк.
    pub logs_scroll: gpui::ScrollHandle,
    /// Панель, под которую создан фильтр-инпут (плейсхолдеры разные).
    pub log_filter_panel: &'static str,
    pub log_filter_sub: Option<gpui::Subscription>,
    /// Применённое (после debounce 150 мс) значение фильтра логов и поколение
    /// ввода — `FILTER_DEBOUNCE_MS` оригинала (`LogsPanel.tsx:17,31-38`).
    pub(crate) log_filter_value: String,
    pub(crate) log_filter_gen: u64,
    /// Кэш иконок расширений: id → data-URL (None = иконки нет).
    pub ext_icons: std::collections::HashMap<String, Option<String>>,
    /// Активный сегмент фильтра System-лога (all|error|warning|info).
    pub syslog_level: &'static str,
    /// Результат разового детекта старого Electron-Bridge.
    pub legacy_bridge: Option<crate::legacy_bridge::BridgeFootprint>,
    /// Идёт удаление старого Bridge: кнопка карточки гаснет и меняет подпись
    /// (`LegacyBridgeCard.tsx:96-99`, ревью ц.19)
    pub legacy_removing: bool,
    /// Что именно происходит при загрузке списка расширений (вместо «Loading…»).
    pub ext_status: String,
    /// Customize-режим (gear): открыт + активная подпанель.
    pub customize_open: bool,
    pub customize_panel: &'static str,
    /// Contributed Customize-страницы (view_id, name, icon) из registry.
    pub customize_pages: Vec<crate::host_link::CzContainer>,
    /// Активная contributed-страница (view_id) при customize_panel=="contrib".
    pub customize_contrib: Option<String>,
    /// Что уже открыто в czShared: гасит повторные navigate на кадрах.
    pub cz_nav_done: Option<String>,
    /// App-prefs хоста: (backgroundToasts, useConptyDll); None = не загружены.
    pub app_prefs: Option<(bool, bool)>,
    /// Расширения хоста; None = не загружены.
    pub extensions: Option<Vec<crate::ui::extensions_panel::ExtDesc>>,
    pub system_log: Vec<crate::output_log::SysEntry>,
}
