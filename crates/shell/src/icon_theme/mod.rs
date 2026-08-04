//! Contributed file-icon themes (`contributes.iconThemes`, B10 1:1 ядро):
//! хост парсит JSON (`kamin:iconTheme:load`, iconPath уже АБСОЛЮТНЫЕ) —
//! резолв имя→иконка здесь, SVG читается gpui напрямую с диска (data-URL
//! канал хоста не нужен). fontCharacter-дефиниции — известный гэп (как в
//! оригинале, plan/25): такие темы дают фолбэк на Catppuccin.

pub mod img;
pub mod lang;
pub mod maps;
pub mod resolve;

pub use crate::icon_theme::img::{file_img, folder_img, set_active};
pub use crate::icon_theme::maps::IconMaps;

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;

/// Распарсенная icon-тема: только iconPath-дефиниции (в АБС путях).
#[derive(Default, Clone)]
pub struct IconTheme {
    pub base: IconMaps,
    /// Оверрайд светлой темы (`light`); `highContrast` не слоим — у нас нет
    /// HC-kind, ветка была бы мёртвым кодом (как и в оригинале).
    pub light: Option<IconMaps>,
    /// id дефиниции → абсолютный путь SVG.
    pub defs: HashMap<String, PathBuf>,
}

impl IconTheme {
    /// Из dok-а `kamin:iconTheme:load` (iconPath абсолютные).
    pub fn parse(doc: &Value) -> Self {
        let defs = doc
            .get("iconDefinitions")
            .and_then(Value::as_object)
            .map(|o| {
                o.iter()
                    .filter_map(|(id, d)| {
                        Some((id.clone(), PathBuf::from(d.get("iconPath")?.as_str()?)))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            base: IconMaps::parse(doc),
            light: doc.get("light").map(IconMaps::parse),
            defs,
        }
    }

    /// Слои карт, от частного к общему: светлый оверрайд (когда UI светлый),
    /// затем сам документ.
    pub(crate) fn layers(&self, light: bool) -> Vec<&IconMaps> {
        match (light, self.light.as_ref()) {
            (true, Some(l)) => vec![l, &self.base],
            _ => vec![&self.base],
        }
    }

    pub(crate) fn path_of(&self, id: &str) -> Option<PathBuf> {
        self.defs.get(id).cloned()
    }

    pub(crate) fn hit(
        &self,
        ls: &[&IconMaps],
        f: impl Fn(&IconMaps) -> Option<&String>,
    ) -> Option<PathBuf> {
        ls.iter()
            .filter_map(|m| f(m))
            .find_map(|id| self.path_of(id))
    }
}

#[cfg(test)]
mod tests {
    use super::IconTheme;
    use serde_json::json;

    fn fixture() -> IconTheme {
        IconTheme::parse(&json!({
            "iconDefinitions": {
                "_ts": {"iconPath": "C:/x/ts.svg"},
                "_test_ts": {"iconPath": "C:/x/test-ts.svg"},
                "_pkg": {"iconPath": "C:/x/pkg.svg"},
                "_file": {"iconPath": "C:/x/file.svg"},
                "_folder": {"iconPath": "C:/x/folder.svg"},
                "_folder_open": {"iconPath": "C:/x/folder-open.svg"},
                "_src": {"iconPath": "C:/x/src.svg"},
                "_root": {"iconPath": "C:/x/root.svg"},
                "_root_named": {"iconPath": "C:/x/root-named.svg"},
                "_rust": {"iconPath": "C:/x/rust.svg"},
                "_light_ts": {"iconPath": "C:/x/light-ts.svg"},
                "_font": {"fontCharacter": "\u{e001}"}
            },
            "file": "_file",
            "folder": "_folder",
            "folderExpanded": "_folder_open",
            "rootFolder": "_root",
            "rootFolderNames": {"app": "_root_named"},
            "fileExtensions": {"ts": "_ts", "test.ts": "_test_ts", "woff": "_font"},
            "fileNames": {"package.json": "_pkg"},
            "folderNames": {"src": "_src"},
            "languageIds": {"rust": "_rust"},
            "light": {"fileExtensions": {"ts": "_light_ts"}}
        }))
    }

    #[test]
    fn file_resolution_order() {
        let t = fixture();
        // fileNames важнее расширений; регистронезависимо
        assert!(
            t.resolve_file("Package.JSON", false)
                .unwrap()
                .ends_with("pkg.svg")
        );
        // Длинный суффикс first («a.test.ts» → test.ts, не ts)
        assert!(
            t.resolve_file("a.test.ts", false)
                .unwrap()
                .ends_with("test-ts.svg")
        );
        assert!(t.resolve_file("b.ts", false).unwrap().ends_with("ts.svg"));
        // Неизвестное → file-дефолт
        assert!(
            t.resolve_file("readme.md", false)
                .unwrap()
                .ends_with("file.svg")
        );
        // fontCharacter-дефиниция без iconPath → фолбэк на дефолт
        assert!(
            t.resolve_file("f.woff", false)
                .unwrap()
                .ends_with("file.svg")
        );
    }

    #[test]
    fn language_id_layer() {
        // `rs` нет в fileExtensions, но есть languageIds["rust"]
        let t = fixture();
        assert!(
            t.resolve_file("main.rs", false)
                .unwrap()
                .ends_with("rust.svg")
        );
    }

    #[test]
    fn light_override_wins() {
        let t = fixture();
        assert!(
            t.resolve_file("b.ts", true)
                .unwrap()
                .ends_with("light-ts.svg")
        );
        // Чего нет в оверрайде — берётся из базы
        assert!(t.resolve_file("b.md", true).unwrap().ends_with("file.svg"));
    }

    #[test]
    fn folder_resolution() {
        let t = fixture();
        assert!(
            t.resolve_folder("SRC", false, false, false)
                .unwrap()
                .ends_with("src.svg")
        );
        assert!(
            t.resolve_folder("lib", false, false, false)
                .unwrap()
                .ends_with("folder.svg")
        );
        assert!(
            t.resolve_folder("lib", true, false, false)
                .unwrap()
                .ends_with("folder-open.svg")
        );
    }

    #[test]
    fn root_folder_maps() {
        let t = fixture();
        // Корень с именем из rootFolderNames
        assert!(
            t.resolve_folder("app", false, true, false)
                .unwrap()
                .ends_with("root-named.svg")
        );
        // Корень без имени → rootFolder, а НЕ folderNames («src» игнорируется)
        assert!(
            t.resolve_folder("src", false, true, false)
                .unwrap()
                .ends_with("root.svg")
        );
    }
}
