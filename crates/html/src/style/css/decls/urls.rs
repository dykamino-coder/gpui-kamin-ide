//! url() и комментарии в значениях: распознавание и пропуск url(), плохие url, вырезание /* */.

use super::*;

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
pub(super) fn has_bad_url(value: &str) -> bool {
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
