//! Модель contributed-дерева: узлы, метаданные, состояние, перетаскивание.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::ui::ctree::types::{EXPANDED, NONE, TreeMeta, TreeNodeDto, TreeViewState};
use serde_json::Value;

impl TreeNodeDto {
    pub fn from_value(v: &Value) -> Option<Self> {
        let handle = v.get("handle").and_then(Value::as_str)?.to_string();
        let s = |k: &str| {
            v.get(k)
                .and_then(Value::as_str)
                .map(std::string::ToString::to_string)
        };
        let command = v.get("command").and_then(|c| {
            let name = c.get("command").and_then(Value::as_str)?.to_string();
            let args = c
                .get("arguments")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            Some((name, args))
        });
        Some(Self {
            handle,
            label: s("label").unwrap_or_default(),
            description: s("description"),
            tooltip: s("tooltip"),
            codicon: s("codicon"),
            resource_uri: s("resourceUri"),
            collapsible: v
                .get("collapsibleState")
                .and_then(Value::as_i64)
                .unwrap_or(NONE),
            checkbox: v.get("checkboxState").and_then(Value::as_i64),
            checkbox_tooltip: s("checkboxTooltip"),
            command,
        })
    }
}
impl TreeMeta {
    pub fn from_value(v: &Value) -> Self {
        let s = |k: &str| {
            v.get(k)
                .and_then(Value::as_str)
                .map(std::string::ToString::to_string)
        };
        let badge = v.get("badge").and_then(|b| {
            let value = b.get("value")?;
            let text = value
                .as_str()
                .map(std::string::ToString::to_string)
                .or_else(|| value.as_i64().map(|n| n.to_string()))?;
            Some((
                text,
                b.get("tooltip")
                    .and_then(Value::as_str)
                    .map(std::string::ToString::to_string),
            ))
        });
        Self {
            title: s("title"),
            description: s("description"),
            message: s("message"),
            badge,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.description.is_none()
            && self.message.is_none()
            && self.badge.is_none()
    }
}
/// Перетаскиваемый узел contributed-дерева (`handleDrag` уже ушёл хосту).
pub struct DraggedTreeNode;
/// Ghost узла у курсора — та же пилюля, что у файлового дерева.
pub struct TreeDragGhost {
    pub label: String,
}
impl TreeViewState {
    /// Уровень раскрыт? Первое появление handle берёт состояние из узла.
    pub fn is_expanded(&self, node: &TreeNodeDto) -> bool {
        self.expanded
            .get(&node.handle)
            .copied()
            .unwrap_or(node.collapsible == EXPANDED)
    }
}
