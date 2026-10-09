//! Объявления: разбор блока, вложенные блоки, экранирование, url(), комментарии, деление аргументов.

use crate::style::css::*;

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

/// Аргументы функции CSS через запятую, не заходя внутрь вложенных скобок:
/// `rgba(0,0,0,.4), inset 0 0 2px red` — два аргумента, а не пять.
pub fn split_args(raw: &str) -> Vec<&str> {
    split_top_level(raw, ',')
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Имя без экранирования (CSS Syntax §4.3.7).
///
/// `BSL0031 ` — знак по шестнадцатеричному коду, до шести цифр, и один
/// пробел после них съедается как ограничитель. `BSL.` — сама точка, а не
/// разделитель составного селектора. Пока этого не было, `p\\.class`
/// разбирался как тег `p` с классом `class` и совпадал с `p class="class"`,
/// хотя обязан искать тег с точкой в имени, то есть не совпадать ни с чем.
/// Кончается ли накопленный кусок НЕЗАВЕРШЁННЫМ hex-экранированием:
/// обратная косая, за ней от одной до шести шестнадцатеричных цифр.
pub(super) fn ends_with_open_escape(cur: &str) -> bool {
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
pub(super) fn unescape_value(value: &str) -> String {
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

/// Чем кончилась очередная запись таблицы.
pub(crate) enum Piece<'a> {
    /// Правило с телом: заголовок и содержимое фигурных скобок.
    Block { head: &'a str, body: &'a str },
    /// At-правило-предложение: заголовок до точки с запятой, тела нет.
    /// Заголовок пока никем не читается, но остаётся в разборе для симметрии.
    #[allow(dead_code)]
    Statement { head: &'a str },
}

/// Есть ли в значении незакавыченная запись `url(…)` с негодным содержимым.
fn has_bad_url(value: &str) -> bool {
    let mut at = 0usize;
    while at < value.len() {
        let ch = value[at..].chars().next().unwrap_or('\0');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += value[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&value[at..], ch);
                continue;
            }
            _ if at_url(&value[at..]) => {
                if url_is_bad(&value[at..]) {
                    return true;
                }
                at += skip_url(&value[at..]);
                continue;
            }
            _ => at += ch.len_utf8(),
        }
    }
    false
}

/// Годен ли url-токен, начавшийся здесь (§4.3.6).
///
/// Незакавыченное содержимое портят кавычка, открывающая скобка, знак
/// управления и непробельный знак после пробела в середине; закавыченная
/// форма — функция со строкой, к токену не относится. Обрыв на конце файла
/// токен НЕ портит.
fn url_is_bad(text: &str) -> bool {
    let mut at = 4; // `url(`
    let bytes = text.as_bytes();
    while at < bytes.len() && bytes[at].is_ascii_whitespace() {
        at += 1;
    }
    if matches!(bytes.get(at), Some(b'"') | Some(b'\'')) {
        return false;
    }
    let mut ws_seen = false;
    while at < text.len() {
        let ch = text[at..].chars().next().unwrap_or('\0');
        match ch {
            ')' => return false,
            '\\' => {
                at += ch.len_utf8();
                at += text[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            c if c.is_ascii_whitespace() => ws_seen = true,
            '"' | '\'' | '(' => return true,
            c if (c as u32) < 0x20 || c as u32 == 0x7f => return true,
            _ if ws_seen => return true,
            _ => {}
        }
        at += ch.len_utf8();
    }
    false
}

/// Начинается ли здесь запись `url(`.
pub(crate) fn at_url(text: &str) -> bool {
    // Сравнение по БАЙТАМ: срез по четвёртому байту может разрезать
    // многобайтовый знак, и обычный срез строки на этом падает.
    let b = text.as_bytes();
    b.len() >= 4 && b[..4].eq_ignore_ascii_case(b"url(")
}

/// Где кончается запись `url(…)`, считая от `u`.
///
/// Незакавыченное содержимое — отдельный вид токена (§4.3.6): фигурная
/// скобка, точка с запятой и начало комментария внутри него ничего не значат.
/// Пока запись разбиралась как обычный текст, `url( { test )` открывал блок,
/// и остаток таблицы съезжал (`uri-012`).
pub(crate) fn skip_url(text: &str) -> usize {
    let mut at = 4; // `url(`
    while at < text.len() {
        let ch = text[at..].chars().next().unwrap_or('\u{0}');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += text[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += skip_string(&text[at..], ch);
                continue;
            }
            ')' => return at + ch.len_utf8(),
            _ => at += ch.len_utf8(),
        }
    }
    text.len()
}

pub(crate) fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut at = 0usize;
    while at < css.len() {
        let ch = css[at..].chars().next().unwrap_or('\0');
        // Кавычки и экранирование сильнее комментария: `content: "/*"` — это
        // текст, а не начало комментария до конца таблицы.
        match ch {
            '\\' => {
                let next = css[at + ch.len_utf8()..].chars().next();
                out.push(ch);
                if let Some(n) = next {
                    out.push(n);
                    at += ch.len_utf8() + n.len_utf8();
                } else {
                    at += ch.len_utf8();
                }
                continue;
            }
            '"' | '\'' => {
                let body = at + ch.len_utf8();
                let end = body + skip_string(&css[body..], ch);
                out.push_str(&css[at..end]);
                at = end;
                continue;
            }
            _ if at_url(&css[at..]) => {
                let end = at + skip_url(&css[at..]);
                out.push_str(&css[at..end]);
                at = end;
                continue;
            }
            _ => {}
        }
        if css[at..].starts_with("/*") {
            // На месте комментария остаётся ПРОБЕЛ: разбор идёт по строке, а
            // не по разборным единицам, и без него соседние значения
            // склеивались в одно (`hsla(120/* … */75%/* … */50%)` выходило
            // `hsla(12075%50%)` — цвет пропадал целиком).
            out.push(' ');
            match css[at + 2..].find("*/") {
                Some(end) => at += 2 + end + 2,
                // Незакрытый комментарий тянется до конца файла (§4.3.2).
                None => return out,
            }
            continue;
        }
        out.push(ch);
        at += ch.len_utf8();
    }
    out
}

/// Разрезание по разделителю, не заходя внутрь скобок: `rgba(0, 0, 0, .5)`
/// содержит запятые, а `grid-template: repeat(2, 1fr)` — и запятые, и скобки.
pub(super) fn split_top_level(raw: &str, sep: char) -> Vec<&str> {
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
