//! Общие лексические помощники: url(), деление вне скобок, склейка через косую черту.

/// `url(...)` из значения фона; кавычки внутри необязательны.
///
/// Имя записи регистронезависимо (§3.3), поэтому `URL(` и `Url(` — та же
/// запись; экранирование в нём разбор объявления уже снял. Конец ищется
/// с учётом кавычек и экранирования: закрывающая скобка внутри строки записи
/// не закрывает (§4.3.6). И искать `url(` внутри строки нельзя вовсе —
/// `content: "url(x)"` записью не является.
pub(crate) fn parse_url(v: &str) -> Option<String> {
    let mut at = 0usize;
    while at < v.len() {
        let ch = v[at..].chars().next().unwrap_or('\u{0}');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += v[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += crate::style::css::skip_string(&v[at..], ch);
                continue;
            }
            _ if crate::style::css::at_url(&v[at..]) => {
                let end = at + crate::style::css::skip_url(&v[at..]);
                // Обрыв на конце файла закрывает запись сам (§4.2): скобки
                // может не быть, и тогда резать последний знак нельзя, а
                // кавычка остаётся только открывающая (`uri-017`).
                let inner_end = if v[..end].ends_with(')') {
                    end - 1
                } else {
                    end
                };
                let inner = v[at + 4..inner_end.max(at + 4)].trim();
                let inner = match inner.chars().next() {
                    Some(q @ ('"' | '\'')) if inner.len() > 1 => {
                        let body = &inner[1..];
                        body.strip_suffix(q).unwrap_or(body)
                    }
                    _ => inner,
                };
                return (!inner.is_empty()).then(|| inner.to_string());
            }
            _ => at += ch.len_utf8(),
        }
    }
    None
}

/// Разбить значение по пробелам, НЕ заходя внутрь скобок.
///
/// `rgba(0, 0, 0, .5)` — это один токен, а не четыре: обычное деление по
/// пробелам разрывало функции с пробелами после запятых, и значение молча
/// пропадало.
pub(crate) fn split_outside_parens(v: &str) -> Vec<String> {
    let mut out = vec![];
    let mut depth = 0usize;
    let mut cur = String::new();
    for ch in v.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                cur.push(ch);
            }
            c if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Голова сокращения `font` и семейство: семейство — хвост после размера.
/// Убрать пробелы вокруг косой черты: `50px / 1` → `50px/1`.
pub(in crate::style::computed) fn join_slash(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for part in v.split('/') {
        if !out.is_empty() {
            out.push('/');
            out.push_str(part.trim_start());
        } else {
            out.push_str(part.trim_end());
        }
    }
    out
}
