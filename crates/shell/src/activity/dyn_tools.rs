//! Динамические тулы и вью расширений: реестр и запросы.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

/// Одно вью контейнера (`contributes.views[]`).
#[derive(Clone, Debug)]
pub struct DynView {
    pub id: String,
    pub name: String,
    /// `contributes.views[].type == "webview"`; иначе — TreeDataProvider-вью.
    pub webview: bool,
}
/// Contributed тул (view-контейнер расширения из registry: activitybar/
/// auxiliarybar): id контейнера, label/icon (codicon-имя) контейнера и ВСЕ
/// его вью — тело панели рисует их стопкой (`ContributedContainerBody`).
#[derive(Clone, Debug)]
pub struct DynTool {
    pub id: String,
    pub label: String,
    pub icon: String,
    /// `viewContainers[].location` — нужна reveal'у, чтобы непринятый
    /// контейнер лёг в слот по умолчанию (`ipc.ts:313`)
    pub location: String,
    pub views: Vec<DynView>,
}
pub(crate) fn dyn_tools() -> &'static std::sync::Mutex<Vec<DynTool>> {
    static S: std::sync::OnceLock<std::sync::Mutex<Vec<DynTool>>> = std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}
pub fn set_dyn_tools(tools: Vec<DynTool>) {
    *dyn_tools().lock().unwrap() = tools;
}
pub fn dyn_tools_list() -> Vec<DynTool> {
    dyn_tools().lock().unwrap().clone()
}
/// Тул-одиночка: существует ровно в одном слоте. Открыть копию в другой
/// панели нельзя — только перетащить. Сейчас это ВСЕ contributed-тулы (вью
/// расширений: вебвью один, второй экземпляр нечем наполнить).
/// Есть ли id в реестре contributed (без клонирования записи).
pub fn dyn_has(id: &str) -> bool {
    dyn_tools().lock().unwrap().iter().any(|t| t.id == id)
}
