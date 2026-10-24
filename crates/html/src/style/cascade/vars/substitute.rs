//! Подстановки токенов CSS Values 5 с сохранением их границ при сериализации.

use super::{CascadeCx, ValueCx, attr_cast::attr_cast};
use crate::css_syntax::{escape_end, functions, unescape};
use crate::style::css::{block_end, split_top_level, strip_comments};

pub(in crate::style::cascade) fn resolve_sibling(value: String, cx: CascadeCx) -> String {
    let Some((index, count)) = cx.sibling else {
        return value;
    };
    let mut out = String::with_capacity(value.len());
    let mut from = 0;
    for f in functions(&value) {
        if f.start < from {
            continue;
        }
        let n = if f.name.eq_ignore_ascii_case("sibling-index") {
            index
        } else if f.name.eq_ignore_ascii_case("sibling-count") {
            count
        } else {
            continue;
        };
        let Some(end) = block_end(&value, f.open, '(') else {
            return "unset".into();
        };
        if !strip_comments(&value[f.open + 1..end]).trim().is_empty() {
            return "unset".into();
        }
        out.push_str(&value[from..f.start]);
        // CSS Variables §3: подстановка не сливает соседние токены.
        out.push_str(&format!(" {n} "));
        from = (end + 1).min(value.len());
    }
    out.push_str(&value[from..]);
    out
}

fn attr_value<'a>(name: &str, cx: CascadeCx<'a>) -> Option<&'a str> {
    let local = match name.split_once('|') {
        Some(("", local)) => local,
        Some(_) => return None,
        None => name,
    };
    cx.attrs
        .iter()
        .find(|(key, _)| {
            if cx.html_attrs {
                key.eq_ignore_ascii_case(local)
            } else {
                key == local
            }
        })
        .map(|(_, value)| value.as_str())
}

fn head_parts(head: &str) -> (String, &str) {
    let mut end = 0;
    while let Some(c) = head[end..].chars().next() {
        if c == '\\' {
            end = escape_end(head, end);
        } else if c.is_whitespace() {
            break;
        } else {
            end += c.len_utf8();
        }
    }
    (unescape(&head[..end]), head[end..].trim())
}

pub(in crate::style::cascade) fn resolve_attrs(
    _key: &str,
    value: &str,
    cx: CascadeCx,
    vcx: ValueCx<'_>,
) -> String {
    attrs(value, cx, vcx, 0, &mut (32 << 20)).unwrap_or_else(|| "unset".into())
}

fn attrs(
    value: &str,
    cx: CascadeCx,
    vcx: ValueCx<'_>,
    depth: usize,
    work: &mut usize,
) -> Option<String> {
    // Бюджет входа ограничивает также рекурсию атрибутов, запасов и циклы.
    if depth > 128 {
        return None;
    }
    *work = work.checked_sub(value.len().checked_mul(8)?)?;
    let mut out = String::with_capacity(value.len());
    let mut from = 0;
    for f in functions(value) {
        if f.start < from || !f.name.eq_ignore_ascii_case("attr") {
            continue;
        }
        let end = block_end(value, f.open, '(')?;
        let inner = &value[f.open + 1..end];
        let args = split_top_level(inner, ',');
        let head = strip_comments(args[0]);
        let (name, ty) = head_parts(head.trim());
        if name.starts_with("*|") {
            return None;
        }
        if ty.is_empty() {
            out.push_str(&value[from..f.start]);
            from = (end + 1).min(value.len());
            out.push_str(&value[f.start..from]);
            continue;
        }
        let fallback = if args.len() > 1 {
            // Аргументы разделяет только первая запятая; остальные принадлежат
            // запасному declaration-value.
            let comma = args[0].len();
            Some(&inner[comma + 1..])
        } else {
            None
        };
        let cast = attr_value(&name, cx).and_then(|v| {
            let expanded = attrs(v, cx, vcx, depth + 1, work)?;
            attr_cast(&expanded, ty, vcx)
        });
        let replacement = match cast {
            Some(v) => v,
            None => attrs(fallback?, cx, vcx, depth + 1, work)?,
        };
        // CSS Values 5 §7.7.2: URL из attr() даёт IACVT, а не ошибку типа
        // с выбором запаса. Строки непрозрачны.
        if functions(&replacement).any(|f| f.name.eq_ignore_ascii_case("url")) {
            return None;
        }
        if out
            .len()
            .checked_add(f.start - from)?
            .checked_add(replacement.len())?
            > (1 << 20) - 2
        {
            return None;
        }
        out.push_str(&value[from..f.start]);
        out.push(' ');
        out.push_str(&replacement);
        out.push(' ');
        from = (end + 1).min(value.len());
    }
    if out.len().checked_add(value.len() - from)? > 1 << 20 {
        return None;
    }
    out.push_str(&value[from..]);
    Some(out)
}
