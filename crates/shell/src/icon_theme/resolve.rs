//! Подбор иконки по имени файла и папки в icon-теме.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::icon_theme::IconTheme;
use crate::icon_theme::lang::EXT_TO_LANGUAGE_ID;
use std::path::PathBuf;

impl IconTheme {
    /// Файл: fileNames → цепочка суффиксов после каждой точки → languageIds по
    /// последнему расширению → file-дефолт.
    pub fn resolve_file(&self, name: &str, light: bool) -> Option<PathBuf> {
        let ls = self.layers(light);
        let lower = name.to_lowercase();
        if let Some(p) = self.hit(&ls, |m| m.file_names.get(&lower)) {
            return Some(p);
        }
        for (i, _) in lower.match_indices('.') {
            let ext = &lower[i + 1..];
            if let Some(p) = self.hit(&ls, |m| m.file_extensions.get(ext)) {
                return Some(p);
            }
        }
        let last_ext = lower.rsplit_once('.').map(|(_, e)| e);
        if let Some(lang) = last_ext.and_then(|e| {
            EXT_TO_LANGUAGE_ID
                .iter()
                .find(|(k, _)| *k == e)
                .map(|(_, v)| *v)
        }) && let Some(p) = self.hit(&ls, |m| m.language_ids.get(lang))
        {
            return Some(p);
        }
        self.hit(&ls, |m| m.file.as_ref())
    }
    /// Папка: folderNames(Expanded) → folder/folderExpanded (fallback друг
    /// на друга, как VS Code при отсутствии expanded-варианта). Корень
    /// рабочей папки берёт карты `rootFolder*` и НЕ проваливается в обычные
    /// `folderNames` (`fileIconThemeData.ts:278-330`).
    pub fn resolve_folder(
        &self,
        name: &str,
        open: bool,
        is_root: bool,
        light: bool,
    ) -> Option<PathBuf> {
        let ls = self.layers(light);
        let lower = name.to_lowercase();
        if is_root {
            if open && let Some(p) = self.hit(&ls, |m| m.root_folder_names_expanded.get(&lower)) {
                return Some(p);
            }
            if let Some(p) = self.hit(&ls, |m| m.root_folder_names.get(&lower)) {
                return Some(p);
            }
            if open
                && let Some(p) = self.hit(&ls, |m| {
                    m.root_folder_expanded
                        .as_ref()
                        .or(m.folder_expanded.as_ref())
                })
            {
                return Some(p);
            }
            return self.hit(&ls, |m| m.root_folder.as_ref().or(m.folder.as_ref()));
        }
        if open && let Some(p) = self.hit(&ls, |m| m.folder_names_expanded.get(&lower)) {
            return Some(p);
        }
        if let Some(p) = self.hit(&ls, |m| m.folder_names.get(&lower)) {
            return Some(p);
        }
        if open {
            return self.hit(&ls, |m| m.folder_expanded.as_ref().or(m.folder.as_ref()));
        }
        self.hit(&ls, |m| m.folder.as_ref())
    }
}
