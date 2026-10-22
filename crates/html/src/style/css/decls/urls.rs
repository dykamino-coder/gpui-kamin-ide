//! url() и комментарии в значениях: распознавание и пропуск url(), плохие url, вырезание /* */.

pub(crate) use crate::css_syntax::{at_url, skip_url, strip_comments};

use super::*;

/// Чем кончилась очередная запись таблицы.
pub(crate) enum Piece<'a> {
    /// Правило с телом: заголовок и содержимое фигурных скобок.
    Block { head: &'a str, body: &'a str },
    /// At-правило-предложение: заголовок до точки с запятой, тела нет.
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
