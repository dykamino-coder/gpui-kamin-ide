//! Диагностика: модель записи и её разбор.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use serde_json::Value;

/// Диагностика (DiagnosticDto → плоско): 0=err 1=warn 2=info 3=hint.
#[derive(Clone)]
pub struct Diag {
    pub severity: u8,
    pub line: u32,
    pub character: u32,
    pub message: String,
    pub source: String,
    pub code: String,
}
impl Diag {
    pub fn from_value(v: &Value) -> Option<Diag> {
        let start = v.get("range").and_then(|r| r.get("start"));
        Some(Diag {
            severity: v.get("severity").and_then(Value::as_u64).unwrap_or(0) as u8,
            line: start
                .and_then(|s| s.get("line"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            character: start
                .and_then(|s| s.get("character"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            message: v.get("message").and_then(Value::as_str)?.to_string(),
            source: v
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            code: match v.get("code") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Number(n)) => n.to_string(),
                _ => String::new(),
            },
        })
    }

    /// «source(code)» / «source» / «code» (ProblemRow.originText 1:1).
    pub(crate) fn origin(&self) -> String {
        if !self.source.is_empty() && !self.code.is_empty() {
            format!("{}({})", self.source, self.code)
        } else if !self.source.is_empty() {
            self.source.clone()
        } else {
            self.code.clone()
        }
    }
}
