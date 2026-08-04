//! Catppuccin-иконки дерева файлов: подбор иконки по имени файла, по
//! расширению и по имени папки.
//!
//! Сами таблицы соответствий — это ДАННЫЕ, а не код: они лежат рядом,
//! в `assets/icons/cat/*.txt` (по строке на запись, поля через табуляцию),
//! и вшиваются `include_str!`. Раньше те же таблицы были сгенерированным
//! `.rs` на 5748 строк — читать его было невозможно, а править бессмысленно
//! (генератор `scripts/cat_icons_to_data.py` показывает происхождение).
//!
//! Второй таблицы папок нет: в сгенерированном коде рядом с `folder_special`
//! жил ещё один `match` на 135 имён, но ВСЕ они уже были в первой таблице,
//! которая проверяется раньше — арматуры были мёртвыми и удалены.
//!
//! Порядок подбора для файла ровно как в оригинале (`file-icons.ts:73-107`):
//! сперва полное имя, затем суффиксы от САМОГО ДЛИННОГО к короткому
//! (`theme.css.ts` находит `css.ts` раньше `ts`).

use std::collections::HashMap;
use std::sync::LazyLock;

/// «имя файла → иконка» (1084 записи).
static FILES: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| parse_pairs(include_str!("../../../assets/icons/cat/files.txt")));
/// «расширение или составной суффикс → иконка» (729 записей).
static EXTS: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| parse_pairs(include_str!("../../../assets/icons/cat/exts.txt")));
/// «папка → (закрытая, открытая)» — у этих папок иконки РАЗНЫЕ (421 запись).
static FOLDERS: LazyLock<HashMap<&'static str, (&'static str, &'static str)>> =
    LazyLock::new(|| parse_triples(include_str!("../../../assets/icons/cat/folders.txt")));

fn parse_pairs(src: &'static str) -> HashMap<&'static str, &'static str> {
    src.lines()
        .filter_map(|line| line.split_once('\t'))
        .collect()
}

fn parse_triples(src: &'static str) -> HashMap<&'static str, (&'static str, &'static str)> {
    src.lines()
        .filter_map(|line| {
            let mut it = line.split('\t');
            Some((it.next()?, (it.next()?, it.next()?)))
        })
        .collect()
}

/// Путь ассета по имени иконки без расширения (`android` → `icons/cat/android.svg`).
/// Строки из таблиц живут столько же, сколько программа, поэтому склейка
/// кешируется и «утекает» ровно один раз на имя.
pub fn asset_path(name: &'static str) -> &'static str {
    use std::sync::Mutex;
    static CACHE: LazyLock<Mutex<HashMap<&'static str, &'static str>>> =
        LazyLock::new(Default::default);
    let mut cache = CACHE.lock().unwrap();
    if let Some(path) = cache.get(name) {
        return path;
    }
    let leaked: &'static str = Box::leak(format!("icons/cat/{name}.svg").into_boxed_str());
    cache.insert(name, leaked);
    leaked
}

/// Иконка файла по его имени.
pub fn file_icon(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if let Some(icon) = FILES.get(lower.as_str()) {
        return asset_path(icon);
    }
    // Суффиксы от длинного к короткому: `a.css.ts` → `css.ts`, потом `ts`
    let mut start = 0usize;
    while let Some(i) = lower[start..].find('.') {
        let suffix = &lower[start + i + 1..];
        if let Some(icon) = EXTS.get(suffix) {
            return asset_path(icon);
        }
        start += i + 1;
    }
    "icons/cat/file.svg"
}

/// Иконка папки. `open` — раскрытый узел дерева.
pub fn folder_icon(name: &str, open: bool) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if let Some((closed, opened)) = FOLDERS.get(lower.as_str()) {
        return asset_path(if open { opened } else { closed });
    }
    if open {
        "icons/cat/folder-open.svg"
    } else {
        "icons/cat/folder.svg"
    }
}

#[cfg(test)]
mod tests {
    use super::{file_icon, folder_icon};

    /// Составные расширения должны находиться раньше последнего сегмента
    /// (регрессия ревью ц.13: `rsplit('.')` делал их недостижимыми).
    #[test]
    fn composite_extensions_resolve() {
        assert_eq!(file_icon("theme.css.ts"), "icons/cat/vanilla-extract.svg");
        assert_eq!(file_icon("main.g.dart"), "icons/cat/dart-generated.svg");
        assert_eq!(file_icon("app.tsx"), file_icon("other.tsx"));
    }

    #[test]
    fn known_names_and_folders() {
        assert_eq!(file_icon("AndroidManifest.xml"), "icons/cat/android.svg");
        assert_eq!(file_icon("что-то.неизвестное"), "icons/cat/file.svg");
        assert_eq!(folder_icon(".cargo", false), "icons/cat/folder-cargo.svg");
        assert_eq!(
            folder_icon(".cargo", true),
            "icons/cat/folder-cargo-open.svg"
        );
        assert_eq!(folder_icon("aud", false), "icons/cat/folder-audio.svg");
        assert_eq!(folder_icon("нет-такой", false), "icons/cat/folder.svg");
        assert_eq!(folder_icon("нет-такой", true), "icons/cat/folder-open.svg");
    }
}
