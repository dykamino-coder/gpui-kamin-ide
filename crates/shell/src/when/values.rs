//! Значения when: числа, регулярки, приведение и сравнение.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use serde_json::Value;

pub(crate) fn num(v: f64) -> Value {
    serde_json::Number::from_f64(v)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}
/// JS-флаги → инлайн-флаги regex-крейта (i/s/m/x поддержаны; g/u — no-op).
pub(crate) fn compile_regex(pattern: &str, flags: &str) -> Result<regex::Regex, String> {
    let inline: String = flags.chars().filter(|c| "isxm".contains(*c)).collect();
    let src = if inline.is_empty() {
        pattern.to_string()
    } else {
        format!("(?{inline}){pattern}")
    };
    regex::Regex::new(&src).map_err(|e| e.to_string())
}
pub(crate) fn coerce_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            // Целые без ".0" — как JS String(2) == "2" (парсер даёт f64)
            match n.as_f64() {
                Some(f) if f.fract() == 0.0 && f.abs() < 9e15 => {
                    format!("{}", f as i64)
                }
                _ => n.to_string(),
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
pub(crate) fn loose_eq(a: &Value, b: &Value) -> bool {
    a == b || coerce_str(a) == coerce_str(b)
}
pub(crate) fn member_of(a: &Value, b: &Value) -> bool {
    match b {
        Value::Array(arr) => arr.iter().any(|x| loose_eq(x, a)),
        // VS Code: quoted comma-separated string RHS — `ext in '.js,.ts'`
        Value::String(s) => s
            .split(',')
            .any(|x| loose_eq(&Value::String(x.trim().to_string()), a)),
        Value::Object(o) => o.contains_key(&coerce_str(a)),
        _ => false,
    }
}
