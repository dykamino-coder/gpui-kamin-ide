//! Состояние QuickPick: элементы, фильтр, выбор.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use serde_json::Value;
use std::collections::HashSet;

const SEPARATOR_KIND: i64 = -1;
/// Элемент пика (QuickPickItemDto, плоско).
#[derive(Clone)]
pub struct QpItem {
    pub label: String,
    pub description: String,
    pub detail: String,
    pub picked: bool,
    pub separator: bool,
    /// `alwaysShow` — пункт обходит фильтр (`QuickPickModal.tsx:42`).
    pub always_show: bool,
}
/// Открытый пик: запрос хоста + выбор.
pub struct QuickPickState {
    pub req_id: u64,
    pub title: Option<String>,
    pub placeholder: Option<String>,
    /// `prompt` из options — строка-пояснение над списком (`.prompt`).
    pub prompt: Option<String>,
    pub can_pick_many: bool,
    pub ignore_focus_out: bool,
    pub items: Vec<QpItem>,
    pub checked: HashSet<usize>,
    /// Гейты фильтра (`QuickPickOptions`): без них описание и детали
    /// в поиске НЕ участвуют.
    pub match_on_description: bool,
    pub match_on_detail: bool,
}
impl QuickPickState {
    /// Индексы ОТФИЛЬТРОВАННЫХ пунктов при запросе `query`
    /// (`QuickPickModal.tsx:36-49`): сепараторы и `alwaysShow` фильтр
    /// обходят, описание и детали участвуют только по своим гейтам.
    /// Один источник правды для списка и для Enter — раньше Enter
    /// резолвил `[0]` мимо фильтра и мимо чекбоксов (ревью ц.25).
    pub fn filtered(&self, query: &str) -> Vec<usize> {
        let q = query.trim().to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                if it.separator || it.always_show || q.is_empty() {
                    return true;
                }
                it.label.to_lowercase().contains(&q)
                    || (self.match_on_description && it.description.to_lowercase().contains(&q))
                    || (self.match_on_detail && it.detail.to_lowercase().contains(&q))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Ответ на Enter: multi → отмеченные, single → ПЕРВЫЙ отфильтрованный
    /// НЕ-сепаратор (`QuickPickModal.tsx:53,83-87`).
    pub fn enter_pick(&self, query: &str) -> Option<Vec<usize>> {
        if self.can_pick_many {
            let mut v: Vec<usize> = self.checked.iter().copied().collect();
            v.sort_unstable();
            return Some(v);
        }
        self.filtered(query)
            .into_iter()
            .find(|i| !self.items[*i].separator)
            .map(|i| vec![i])
    }

    /// Из shell.showQuickPick params: (items, options).
    pub fn from_request(req_id: u64, items: &Value, options: &Value) -> QuickPickState {
        let parsed: Vec<QpItem> = items
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|v| QpItem {
                        label: v.get("label").and_then(Value::as_str).unwrap_or("").into(),
                        description: v
                            .get("description")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .into(),
                        detail: v.get("detail").and_then(Value::as_str).unwrap_or("").into(),
                        picked: v.get("picked").and_then(Value::as_bool).unwrap_or(false),
                        separator: v.get("kind").and_then(Value::as_i64) == Some(SEPARATOR_KIND),
                        always_show: v
                            .get("alwaysShow")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let checked = parsed
            .iter()
            .enumerate()
            .filter(|(_, it)| it.picked)
            .map(|(i, _)| i)
            .collect();
        let get_s = |k: &str| options.get(k).and_then(Value::as_str).map(String::from);
        QuickPickState {
            req_id,
            title: get_s("title"),
            placeholder: get_s("placeHolder"),
            prompt: get_s("prompt"),
            match_on_description: options
                .get("matchOnDescription")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            match_on_detail: options
                .get("matchOnDetail")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            can_pick_many: options
                .get("canPickMany")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            ignore_focus_out: options
                .get("ignoreFocusOut")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            items: parsed,
            checked,
        }
    }
}
