//! Раскрытие экранирования имён по CSS Syntax §4.3.7.

pub fn unescape(name: &str) -> String {
    if !name.contains('\\') {
        return name.to_string();
    }
    let mut out = String::with_capacity(name.len());
    let mut it = name.chars().peekable();
    while let Some(ch) = it.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let mut hex = String::new();
        while hex.len() < 6 {
            match it.peek() {
                Some(c) if c.is_ascii_hexdigit() => {
                    hex.push(*c);
                    it.next();
                }
                _ => break,
            }
        }
        if hex.is_empty() {
            // Экранирован обычный знак — он и остаётся, уже без особого
            // значения. Перевод строки экранировать нельзя, но в имени его и
            // не бывает.
            if let Some(c) = it.next() {
                out.push(c);
            }
            continue;
        }
        // Один пробел после цифр — ограничитель кода, а не часть имени.
        if it.peek().is_some_and(|c| c.is_whitespace()) {
            it.next();
        }
        match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
            // Нулевой знак и суррогаты заменяются знаком замены (§4.3.7).
            Some(c) if c != '\u{0}' => out.push(c),
            _ => out.push('\u{fffd}'),
        }
    }
    out
}

