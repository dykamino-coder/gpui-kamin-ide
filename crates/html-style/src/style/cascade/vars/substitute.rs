//! Подстановки токенов CSS Values 5 с сохранением их границ при сериализации.

use super::CascadeCx;
use crate::css_syntax::functions;
use crate::style::css::{block_end, strip_comments};

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

pub(in crate::style::cascade) use super::attr_expansion::resolve_attrs;
