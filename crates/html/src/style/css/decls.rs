//! Объявления: разбор блока, вложенные блоки, экранирование, url(), комментарии, деление
//! аргументов.

use crate::style::css::*;

mod continuation;
mod lexical;
mod urls;
use continuation::join_string_continuations;
pub(crate) use lexical::split_top_level;
pub(super) use lexical::{ends_with_open_escape, unescape_value};
pub use lexical::{split_args, unescape};
use urls::has_bad_url;
pub(crate) use urls::{Piece, at_url, skip_url, strip_comments};

/// Разбор `style="a: 1; b: 2"`.
pub fn parse_decls(raw: &str) -> DeclBlock {
    let raw = component_tokens::complete(raw);
    let mut out = DeclBlock::new();
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
        let joined = join_string_continuations(v);
        let val = joined.trim();
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
        // Пометка важности — флаг объявления, значение хранится без неё.
        let important = top_level_bang(val).is_some();
        let val = font_family_values::normalize(&key, value);
        if !key.is_empty() && (custom || !val.is_empty()) {
            // Повтор того же свойства НЕ затирает прежнее на разборе:
            // действительность значения известна только применению
            // (CSS 2.1 §4.1.7 — недействительное объявление игнорируется,
            // а не гасит предыдущее). Повтор — отдельная запись блока на
            // своём месте.
            out.push(key, val, important);
        }
    }
    out
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
