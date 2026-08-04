//! Карты icon-темы: разбор JSON в соответствия.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use serde_json::Value;
use std::collections::HashMap;

/// Набор карт темы: он одинаков у корня документа и у оверрайда `light`
/// (оверрайд ссылается на ТЕ ЖЕ `iconDefinitions`).
#[derive(Default, Clone)]
pub struct IconMaps {
    pub file: Option<String>,
    pub folder: Option<String>,
    pub folder_expanded: Option<String>,
    pub root_folder: Option<String>,
    pub root_folder_expanded: Option<String>,
    pub file_extensions: HashMap<String, String>,
    pub file_names: HashMap<String, String>,
    pub folder_names: HashMap<String, String>,
    pub folder_names_expanded: HashMap<String, String>,
    pub root_folder_names: HashMap<String, String>,
    pub root_folder_names_expanded: HashMap<String, String>,
    pub language_ids: HashMap<String, String>,
}
fn str_map(v: Option<&Value>) -> HashMap<String, String> {
    v.and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| Some((k.to_lowercase(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}
impl IconMaps {
    pub(crate) fn parse(doc: &Value) -> Self {
        let s = |k: &str| doc.get(k).and_then(Value::as_str).map(str::to_string);
        Self {
            file: s("file"),
            folder: s("folder"),
            folder_expanded: s("folderExpanded"),
            root_folder: s("rootFolder"),
            root_folder_expanded: s("rootFolderExpanded"),
            file_extensions: str_map(doc.get("fileExtensions")),
            file_names: str_map(doc.get("fileNames")),
            folder_names: str_map(doc.get("folderNames")),
            folder_names_expanded: str_map(doc.get("folderNamesExpanded")),
            root_folder_names: str_map(doc.get("rootFolderNames")),
            root_folder_names_expanded: str_map(doc.get("rootFolderNamesExpanded")),
            language_ids: str_map(doc.get("languageIds")),
        }
    }
}
