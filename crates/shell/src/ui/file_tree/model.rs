//! Модель дерева файлов: записи, декорации, состояние, пути.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use kamin_theme::Palette;
use std::collections::{BTreeSet, HashMap};

pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}
/// FileDecorationDto хоста (badge + ThemeColor id); tooltip не показываем.
#[derive(Clone)]
pub struct Deco {
    pub badge: Option<String>,
    pub color: Option<String>,
    /// `FileDecoration.tooltip` — подсказка строки и бейджа
    /// (`data-tooltip={deco?.tooltip ?? path}`, ревью ц.14).
    pub tooltip: Option<String>,
}
/// ThemeColor id → цвет палитры (COLOR_MAP file-decorations.ts 1:1,
/// fallback accent_blue как в оригинале).
pub fn deco_color(id: &str, p: &Palette) -> kamin_theme::Color {
    match id {
        "gitDecoration.modifiedResourceForeground" => p.accent_orange,
        "gitDecoration.untrackedResourceForeground"
        | "gitDecoration.addedResourceForeground"
        | "gitDecoration.stageModifiedResourceForeground" => p.accent_green,
        "gitDecoration.deletedResourceForeground"
        | "gitDecoration.conflictingResourceForeground" => p.accent_red,
        "gitDecoration.ignoredResourceForeground" => p.text_disabled,
        "gitDecoration.submoduleResourceForeground" => p.accent_blue,
        "list.errorForeground" | "problemsErrorIcon.foreground" => p.accent_red,
        "list.warningForeground" | "problemsWarningIcon.foreground" => p.accent_yellow,
        _ => p.accent_blue,
    }
}
/// Состояние дерева: кэш листингов + раскрытые пути + ожидающие листинга
/// (spinner) (владеет RootView).
#[derive(Default)]
pub struct TreeState {
    /// Программный скролл дерева (Locate → scroll-to-row).
    pub scroll: gpui::ScrollHandle,
    pub cache: HashMap<String, Vec<DirEntry>>,
    pub expanded: BTreeSet<String>,
    /// Пути с запрошенным, но не пришедшим листингом → chevron-spinner.
    pub loading: BTreeSet<String>,
    /// Директории, где юзер раскрыл усечённый листинг («Show N more»).
    /// Текущий кап детей по директории («Show N more» добавляет шаг).
    pub child_cap: std::collections::HashMap<String, usize>,
    /// Хост индексирует воркспейс (kamin:index:status) — бейдж в хедере.
    pub indexing: bool,
    /// Мультиселект: выбранные пути (клик = один, Ctrl+клик = toggle).
    pub selected: BTreeSet<String>,
    /// Якорь Shift-диапазона ( из `file-selection.ts`).
    pub anchor: Option<String>,
    /// FileDecorationProvider-кэш: path → Some(deco)|None («запрошено, пусто»).
    pub deco: HashMap<String, Option<Deco>>,
    /// `.flash` после Locate: путь строки + счётчик запусков (ключ анимации,
    /// иначе повторный Locate по тому же файлу не перезапускает вспышку).
    pub flash: Option<(String, u64)>,
}
/// Кап детей директории до «Show N more» и шаг добавления
/// (оригинал FileTreeView.tsx: `TREE_CHILD_CAP = 100`, `TREE_CHILD_STEP = 200`).
pub const DIR_RENDER_CAP: usize = 100;
pub const DIR_RENDER_STEP: usize = 200;
/// Текущий кап детей `dir` (база 100 + нажатия «Show N more»).
pub fn cap_for(tree: &TreeState, dir: &str) -> usize {
    tree.child_cap.get(dir).copied().unwrap_or(DIR_RENDER_CAP)
}
/// Сравнение путей без учёта регистра и вида слэшей: пути дерева строятся
/// из листингов хоста, пути табов редактора приходят из его ответов, и
/// регистр диска у них может отличаться — сравнение «как есть» рвало
/// выделение (ревью ц.12).
pub fn same_path(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.replace('/', "\\").to_lowercase();
    norm(a) == norm(b)
}
pub fn join(dir: &str, name: &str) -> String {
    let sep = if dir.ends_with('\\') || dir.ends_with('/') {
        ""
    } else {
        "\\"
    };
    format!("{dir}{sep}{name}")
}
/// Видимый порядок путей — `visibleOrder()` оригинала: обход ровно как в
/// `rows()`, включая кап уровня. Нужен для Shift-диапазона.
pub fn visible_order(tree: &TreeState, root: &str) -> Vec<String> {
    fn walk(tree: &TreeState, dir: &str, out: &mut Vec<String>) {
        let Some(entries) = tree.cache.get(dir) else {
            return;
        };
        let cap = cap_for(tree, dir);
        let visible: &[DirEntry] = if entries.len() > cap {
            &entries[..cap]
        } else {
            entries
        };
        for e in visible {
            let path = join(dir, &e.name);
            out.push(path.clone());
            if e.is_dir && tree.expanded.contains(&path) {
                walk(tree, &path, out);
            }
        }
    }
    let mut out = vec![root.to_string()];
    if tree.expanded.contains(root) {
        walk(tree, root, &mut out);
    }
    out
}
#[allow(clippy::too_many_arguments)]
/// Индекс строки target в плоском порядке rows() (root row = 0) — для
/// scroll-to-row в Locate. Повторяет порядок/кап rows().
pub fn flat_row_index(tree: &TreeState, root: &str, target: &str) -> Option<usize> {
    fn walk(tree: &TreeState, dir: &str, target: &str, idx: &mut usize) -> Option<usize> {
        let entries = tree.cache.get(dir)?;
        let cap = cap_for(tree, dir);
        let capped = entries.len() > cap;
        let visible: &[DirEntry] = if capped { &entries[..cap] } else { entries };
        for e in visible {
            let path = join(dir, &e.name);
            *idx += 1;
            if same_path(&path, target) {
                return Some(*idx);
            }
            if e.is_dir
                && tree.expanded.contains(&path)
                && let Some(found) = walk(tree, &path, target, idx)
            {
                return Some(found);
            }
        }
        if capped {
            *idx += 1; // строка «Show N more»
        }
        None
    }
    if same_path(root, target) {
        return Some(0);
    }
    if !tree.expanded.contains(root) {
        return None;
    }
    let mut idx = 0usize;
    walk(tree, root, target, &mut idx)
}
