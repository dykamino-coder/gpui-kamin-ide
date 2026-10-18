//! Whitespace-separated CSS value tokens with nested parentheses intact.

/// Разбить запись по пробелам ВЕРХНЕГО уровня: `calc(10px + 15%)` — один
/// токен. Без скобок — ровно `split_whitespace`.
pub(crate) fn split_top(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start: Option<usize> = None;
    for (i, ch) in s.char_indices() {
        if ch.is_whitespace() && depth <= 0 {
            if let Some(st) = start.take() {
                out.push(&s[st..i]);
            }
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        start.get_or_insert(i);
    }
    if let Some(st) = start {
        out.push(&s[st..]);
    }
    out
}
