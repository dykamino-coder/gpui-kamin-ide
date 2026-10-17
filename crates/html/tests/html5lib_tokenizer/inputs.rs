//! Decode tokenizer fixture escapes and map fixture names to tokenizer states.

use html5ever::tokenizer::states;
use serde_json::Value;

/// Состояние токенизатора по имени из набора.
pub(super) fn state_by_name(name: &str) -> states::State {
    match name {
        "PLAINTEXT state" => states::Plaintext,
        "RCDATA state" => states::RawData(states::Rcdata),
        "RAWTEXT state" => states::RawData(states::Rawtext),
        "Script data state" => states::RawData(states::ScriptData),
        "CDATA section state" => states::CdataSection,
        _ => states::Data,
    }
}

/// Двойное экранирование: набор прячет непечатаемое в `\uXXXX`.
pub(super) fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                match u32::from_str_radix(&hex, 16) {
                    Ok(code) => out.push(char::from_u32(code).unwrap_or('\u{fffd}')),
                    Err(_) => out.push_str(&hex),
                }
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Тот же разбор для ожидаемых токенов: строки внутри тоже экранированы.
pub(super) fn unescape_value(value: &Value) -> Value {
    match value {
        Value::String(s) => Value::String(unescape(s)),
        Value::Array(items) => Value::Array(items.iter().map(unescape_value).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (unescape(k), unescape_value(v)))
                .collect(),
        ),
        other => other.clone(),
    }
}
