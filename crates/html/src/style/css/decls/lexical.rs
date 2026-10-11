//! Лексика объявлений: экранирование (unescape*), деление аргументов и списков на верхнем уровне скобок.

use super::*;

/// Имя без экранирования (CSS Syntax §4.3.7).
///
/// `BSL0031 ` — знак по шестнадцатеричному коду, до шести цифр, и один
/// пробел после них съедается как ограничитель. `BSL.` — сама точка, а не
/// разделитель составного селектора. Пока этого не было, `p\\.class`
/// разбирался как тег `p` с классом `class` и совпадал с `p class="class"`,
/// хотя обязан искать тег с точкой в имени, то есть не совпадать ни с чем.
/// Кончается ли накопленный кусок НЕЗАВЕРШЁННЫМ hex-экранированием:
/// обратная косая, за ней от одной до шести шестнадцатеричных цифр.
pub(in crate::style::css) fn ends_with_open_escape(cur: &str) -> bool {
    let hex_len = cur
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_hexdigit())
        .count();
    if hex_len == 0 || hex_len > 6 {
        return false;
    }
    let mut rest = cur.chars().rev().skip(hex_len);
    // Косая перед цифрами, и она сама не экранирована.
    rest.next() == Some('\\') && rest.next() != Some('\\')
}

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

/// Значение без экранирования — но кавычки не трогая.
///
/// Раскрывается всё, что раскрывается в имени, кроме внутренности строк:
/// `"\\""` — это кавычка ВНУТРИ строки, и раскрыв её, мы получили бы три
/// кавычки подряд и порвали значение (`escapes-001`).
///
/// Обрезать результат НЕЛЬЗЯ: `\\0020yellow` раскрывается в имя с пробелом
/// внутри, а такое значение недействительно; обрезка сделала бы из него
/// `yellow` и применила то, что применять нечего (`escapes-014`).
pub(in crate::style::css) fn unescape_value(value: &str) -> String {
    if !value.contains('\\') {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut at = 0usize;
    while at < value.len() {
        let ch = value[at..].chars().next().unwrap_or('\u{0}');
        if ch == '"' || ch == '\'' {
            let body = at + ch.len_utf8();
            let end = body + skip_string(&value[body..], ch);
            out.push_str(&value[at..end]);
            at = end;
            continue;
        }
        if ch != '\\' {
            out.push(ch);
            at += ch.len_utf8();
            continue;
        }
        let tail = &value[at + ch.len_utf8()..];
        let taken = first_escape(tail);
        let one = unescape(&format!("\\{taken}"));
        // Пробел, полученный из кода, — часть ИМЕНИ, а не отступ, и
        // значение с таким именем недействительно. Наш конвейер
        // обрезает значение при использовании, поэтому раскрытие
        // потеряло бы ровно ту особенность, из-за которой объявление и
        // должно отпасть (`escapes-014`, `color:\\0020yellow`).
        if one.chars().all(char::is_whitespace) {
            out.push(ch);
            out.push_str(taken);
        } else {
            out.push_str(&one);
        }
        at += ch.len_utf8() + taken.len();
    }
    out
}

/// Сколько байт после обратного слэша съедает одно экранирование: до шести
/// шестнадцатеричных цифр и один пробел за ними, либо ровно один знак.
fn first_escape(tail: &str) -> &str {
    let mut end = 0usize;
    let mut digits = 0usize;
    for (i, ch) in tail.char_indices() {
        if digits < 6 && ch.is_ascii_hexdigit() {
            digits += 1;
            end = i + ch.len_utf8();
            continue;
        }
        if digits > 0 && ch.is_whitespace() {
            end = i + ch.len_utf8();
        }
        break;
    }
    if digits == 0 {
        end = tail.chars().next().map_or(0, char::len_utf8);
    }
    &tail[..end]
}

/// Аргументы функции CSS через запятую, не заходя внутрь вложенных скобок:
/// `rgba(0,0,0,.4), inset 0 0 2px red` — два аргумента, а не пять.
pub fn split_args(raw: &str) -> Vec<&str> {
    split_top_level(raw, ',')
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Разрезание по разделителю, не заходя внутрь скобок: `rgba(0, 0, 0, .5)`
/// содержит запятые, а `grid-template: repeat(2, 1fr)` — и запятые, и скобки.
pub(in crate::style::css) fn split_top_level(raw: &str, sep: char) -> Vec<&str> {
    let mut out = vec![];
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut at = 0usize;
    while at < raw.len() {
        let ch = raw[at..].chars().next().unwrap_or('\0');
        match ch {
            // Экранированный разделитель разделителем не служит:
            // `background: red\;` — одно объявление со значением `red;`.
            '\\' => {
                at += ch.len_utf8();
                at += raw[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&raw[at..], ch);
                continue;
            }
            _ if at_url(&raw[at..]) => {
                at += skip_url(&raw[at..]);
                continue;
            }
            // Блоки ЛЮБОГО вида непрозрачны: объявление с фигурными скобками
            // внутри (`test { :nested; color: yellow }`) — одно объявление, и
            // недействительное. Пока считались только круглые скобки, его
            // внутренности разбирались как отдельные объявления и применялись
            // (`core-syntax-001`).
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = (depth - 1).max(0),
            c if c == sep && depth == 0 => {
                out.push(&raw[start..at]);
                start = at + ch.len_utf8();
            }
            _ => {}
        }
        at += ch.len_utf8();
    }
    out.push(&raw[start..]);
    out
}
