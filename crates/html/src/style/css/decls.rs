//! Объявления: разбор блока, вложенные блоки, экранирование, url(), комментарии, деление аргументов.

use crate::style::css::*;

mod lexical;
mod urls;
pub(super) use lexical::{ends_with_open_escape, split_top_level, unescape_value};
pub use lexical::{split_args, unescape};
use urls::has_bad_url;
pub(crate) use urls::{Piece, at_url, skip_url, strip_comments};

/// Разбор `style="a: 1; b: 2"`.
pub fn parse_decls(raw: &str) -> Decls {
    let raw = component_tokens::complete(raw);
    let mut out = Decls::new();
    let mut order: Vec<String> = Vec::new();
    for item in split_top_level(&raw, ';') {
        // Двоеточие ищется НЕэкранированное: `bac\\kground` — это имя
        // `background`, а `background\\:` — имя с двоеточием внутри, то есть
        // объявление без двоеточия вовсе, и его надо отбросить
        // (`escapes-002`, `escapes-003`).
        let mut colon = None;
        let mut escaped = false;
        for (i, ch) in item.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                ':' => {
                    colon = Some(i);
                    break;
                }
                _ => {}
            }
        }
        let Some(colon) = colon else {
            continue;
        };
        let (k, v) = (&item[..colon], &item[colon + 1..]);
        let name = unescape(k.trim());
        if name.starts_with('\u{2}') {
            continue;
        }
        let custom = name.starts_with("--");
        if custom && (name == "--" || !selector_tokens::ident(k.trim())) {
            continue;
        }
        // CSS Variables §2: custom-property names are case-sensitive tokens.
        let key = if custom {
            name
        } else {
            name.to_ascii_lowercase()
        };
        // После восклицательного знака в объявлении стоит ровно `important` и
        // ничего больше; всё прочее делает объявление недействительным, и
        // отбрасывается оно целиком (CSS 2.1 §4.1.8). Пока пометка просто
        // срезалась с конца, `background: red ! fail` доезжало значением
        // `red ! fail`, а разбор цвета брал из него первое слово и красил
        // (`core-syntax-006`).
        let val = v.trim();
        if let Some(bang) = top_level_bang(val)
            && !val[bang + 1..].trim().eq_ignore_ascii_case("important")
        {
            continue;
        }
        // Объявление, в чьём значении лежит НЕгодный url-токен, отбрасывается
        // целиком (CSS Syntax §4.3.6): «доехавшая» часть вроде `red` из
        // `background: red url( { test )` красить не должна (`uri-012`).
        if has_bad_url(val) {
            continue;
        }
        let value = top_level_bang(val).map_or(val, |at| val[..at].trim_end());
        if !variable_tokens::valid(value) {
            continue;
        }
        // У обычных свойств пометка важности ОСТАЁТСЯ в значении: снимет её тот, кто раскладывает
        // каскад (`Computed::resolve_with_vars`), а срезав её здесь, мы теряли
        // важность целиком — объявление конкурировало на общих основаниях.
        let important = top_level_bang(val).is_some();
        let val = &font_family_values::normalize(&key, if custom { value } else { val });
        if !key.is_empty() && (custom || !val.is_empty()) {
            // Повтор того же свойства НЕ затирает прежнее на разборе:
            // действительность значения известна только применению
            // (CSS 2.1 §4.1.7 — недействительное объявление игнорируется,
            // а не гасит предыдущее). Части склеиваются служебным
            // разделителем и применяются по порядку. У пользовательских
            // свойств синтаксис уже проверен — последнее побеждает.
            if key.starts_with("--") {
                custom_properties::store(&mut out, key, val.to_string(), important);
            } else {
                // Порядок записи: имя запоминается при ПЕРВОМ появлении —
                // повтор того же свойства применяется на его месте, внутри
                // склеенного значения. В словарь список кладётся ПОСЛЕ
                // разбора и только к непустому: `parse_decls(...).is_empty()`
                // отличает сломанный синтаксис от целого (`@supports`,
                // `@page`), и служебный ключ не должен делать пустое
                // непустым.
                if !out.contains_key(&key) {
                    order.push(key.clone());
                }
                match out.entry(key) {
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        let s = e.get_mut();
                        s.push(DECL_SEP);
                        s.push_str(val);
                    }
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(val.to_string());
                    }
                }
            }
        }
    }
    if !out.is_empty() && !order.is_empty() {
        out.insert(ORDER_KEY.to_string(), order.join(&DECL_SEP.to_string()));
    }
    out
}

/// Объявления блока В ПОРЯДКЕ ЗАПИСИ, повтор свойства — отдельной парой
/// (`Decls` порядок помнит только в служебном `ORDER_KEY`).
pub(super) fn ordered_decls(decls: &Decls) -> Vec<(String, String)> {
    let order = decls.get(ORDER_KEY).cloned().unwrap_or_default();
    order
        .split(DECL_SEP)
        .filter_map(|k| decls.get(k).map(|v| (k, v)))
        .flat_map(|(k, v)| {
            v.split(DECL_SEP)
                .map(move |one| (k.to_string(), one.trim().to_string()))
        })
        .collect()
}

/// Вложенные at-блоки тела `@page`: `(имя без @ в нижнем регистре, тело)`.
pub(super) fn nested_blocks(body: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = body.as_bytes();
    let mut i = 0usize;
    let mut depth = 0usize;
    while i < b.len() {
        match b[i] {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b'@' if depth == 0 => {
                let start = i + 1;
                let Some(open) = body[start..].find('{').map(|k| start + k) else {
                    break;
                };
                let name = body[start..open].trim().to_ascii_lowercase();
                let mut d = 0usize;
                let mut j = open;
                while j < b.len() {
                    match b[j] {
                        b'{' => d += 1,
                        b'}' => {
                            d -= 1;
                            if d == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                out.push((name, body[open + 1..j.min(b.len())].to_string()));
                i = j + 1;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// Срезать вложенные at-блоки из тела `@page`: остаются только объявления.
pub(super) fn strip_nested_blocks(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut depth = 0usize;
    for ch in body.chars() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
        if depth == 1 && ch == '{' {
            // начало вложенного блока: выкинуть его @-голову из хвоста out
            if let Some(at) = out.rfind('@') {
                out.truncate(at);
            }
        }
    }
    out
}
