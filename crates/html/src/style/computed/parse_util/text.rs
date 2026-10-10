//! Разбор текста: деление по пробелам верхнего уровня, схлопывание переводов строк, экранирование в content, кавычки.

/// Разбить значение по пробелам ВНЕ скобок: `rgba(0, 128, 0, .5)` —
/// один токен, а `split_whitespace` рассыпал его, и цвет пропадал
/// (`outline` с функциональным цветом).
pub(in crate::style::computed) fn split_ws_top(v: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, None::<usize>);
    for (i, ch) in v.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = (depth - 1).max(0),
            _ => {}
        }
        if ch.is_whitespace() && depth == 0 {
            if let Some(st) = start.take() {
                out.push(&v[st..i]);
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(st) = start {
        out.push(&v[st..]);
    }
    out
}

/// Разрывы сегмента в строке-маркере: ряд принудительных разрывов — один
/// пробел (css-text-3 §4.1.2, «Segment Break Transformation Rules»).
/// Строка знака обрыва (`text-overflow: <string>`, `block-ellipsis`): ряд
/// ПРИНУДИТЕЛЬНЫХ разрывов — один пробел. Принудительный разрыв по
/// css-text-3 §forced-line-break — сохранённый перевод строки и любой знак
/// классов UAX#14 BK/NL: VT, FF, NEL, LS, PS (Blink `line_truncator.cc`
/// `IsForcedLineBreak`/`SuppressLineBreaks`; `text-overflow-string-018…022`).
pub(in crate::style::computed) fn collapse_forced_breaks(text: &str) -> String {
    let forced = |c: char| {
        matches!(
            c,
            '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
        )
    };
    if !text.contains(forced) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut in_break = false;
    for ch in text.chars() {
        if forced(ch) {
            if !in_break {
                out.push(' ');
                in_break = true;
            }
        } else {
            in_break = false;
            out.push(ch);
        }
    }
    out
}

pub(in crate::style::computed) fn collapse_segment_breaks(text: &str) -> String {
    if !text.contains(['\n', '\r']) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut in_break = false;
    for ch in text.chars() {
        if matches!(ch, '\n' | '\r') {
            if !in_break {
                out.push(' ');
                in_break = true;
            }
        } else {
            in_break = false;
            out.push(ch);
        }
    }
    out
}

pub(in crate::style::computed) fn unescape_content(text: &str) -> String {
    if !text.contains('\\') {
        return text.to_string();
    }
    // Экранированный перевод строки внутри строки CSS — ПРОДОЛЖЕНИЕ, а не
    // знак: CSS 2.1 §4.1.3 «the newline itself has to be escaped with a
    // backslash (\). The newline is subsequently removed from the string»
    // (css-syntax-3 §4.3.5: newline после `\` потребляется). `css::unescape`
    // общий с именами и оставлял `\n`; в `white-space: pre` он становился
    // жёстким разрывом, и кошка `content-173` рвалась лишними строками.
    // Остальные экранирования уходят в `unescape` как есть; `\` в конце
    // строки (EOF) пропадает.
    let mut joined = String::with_capacity(text.len());
    let mut it = text.chars().peekable();
    while let Some(ch) = it.next() {
        if ch != '\\' {
            joined.push(ch);
            continue;
        }
        match it.next() {
            Some('\n') | Some('\u{c}') => {}
            Some('\r') => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
            }
            Some(c) => {
                joined.push('\\');
                joined.push(c);
            }
            None => {}
        }
    }
    crate::style::css::unescape(&joined)
}

/// Кавычка вокруг имени шрифта: `font-family: "Segoe UI", sans-serif`.
pub(in crate::style::computed) fn is_quote(c: char) -> bool {
    c == '"' || c == '\''
}
