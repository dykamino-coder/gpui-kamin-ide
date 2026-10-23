//! Удаление комментариев вне строк и URL по CSS Syntax §4.3.2.

use super::{at_url, skip_string, skip_url};

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
