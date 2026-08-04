//! Модель файлового меню: пункты, вклады расширений, гейт when.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

/// `nameError` оригинала (`signals/file-context-menu.ts:57-63`) — тексты
/// ошибок дословные; гоняется на каждый ввод prompt-модалки.
pub fn name_error(v: &str) -> Option<&'static str> {
    let t = v.trim();
    if t.is_empty() {
        return Some("Name required");
    }
    if t.contains('/') || t.contains('\\') {
        return Some("Name cannot contain path separators");
    }
    if t == ".." || t == "." {
        return Some("Invalid name");
    }
    None
}
/// Открытое меню: узел + позиция; open_in — развёрнут ли «Open In ▸».
#[derive(Clone)]
pub struct FileMenu {
    pub path: String,
    pub is_dir: bool,
    pub x: f32,
    pub y: f32,
    pub open_in: bool,
    /// Мультиселект: кликнутый путь входит в выбор из >1 — Delete по всем.
    pub multi: Vec<String>,
}
/// Contributed explorer/context пункт (из registry:snapshot хоста).
#[derive(Clone)]
pub struct ContribMenuItem {
    pub command: String,
    pub label: String,
    pub group: String,
    pub when: String,
}
/// when-фильтрация полным движком (crate::when, порт when-clause.ts):
/// контекст explorer-узла — explorerResourceIsFolder + resourceFilename /
/// resourceExtname / resourceScheme (как строит ContributedMenu оригинала).
pub(crate) fn when_allows(when: &str, path: &str, is_dir: bool) -> bool {
    use serde_json::json;
    let name = base_name(path);
    let ext = name
        .rfind('.')
        .filter(|i| *i > 0)
        .map(|i| name[i..].to_string())
        .unwrap_or_default();
    let mut ctx = crate::when::ContextValues::new();
    ctx.insert("explorerResourceIsFolder".into(), json!(is_dir));
    ctx.insert("resourceFilename".into(), json!(name));
    ctx.insert("resourceExtname".into(), json!(ext));
    ctx.insert("resourceScheme".into(), json!("file"));
    crate::when::evaluate_when(when, &ctx)
}
/// Порядок VS Code: navigation первым, дальше группы лексикографически,
/// внутри группы по `@N`.
pub(crate) fn group_key(g: &str) -> (String, i64) {
    let mut parts = g.splitn(2, '@');
    let name = parts.next().unwrap_or("");
    let ord = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (
        if name == "navigation" {
            " ".into()
        } else {
            name.to_string()
        },
        ord,
    )
}
/// Имя файла из пути (для seed переименования).
pub(crate) fn base_name(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_string()
}
