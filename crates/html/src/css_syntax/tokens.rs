//! Границы непрозрачных токенов CSS: строка, url() и escape.

use crate::budget::admission::{Admission, Error};

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
    4 + skip_url_body(&text[4..])
}

pub(crate) fn try_skip_url(text: &str, admission: &Admission) -> Result<usize, Error> {
    admission.work(4)?;
    Ok(4 + try_skip_url_body(&text[4..], admission)?)
}

/// Длина тела URL вместе с закрывающей скобкой.
pub(crate) fn skip_url_body(text: &str) -> usize {
    try_skip_url_body(text, &Admission::default()).expect("unlimited token scan")
}

fn try_skip_url_body(text: &str, admission: &Admission) -> Result<usize, Error> {
    let mut at = 0;
    while at < text.len() {
        admission.work(1)?;
        let ch = text[at..].chars().next().unwrap_or('\u{0}');
        match ch {
            '\\' => {
                admission.work(1)?;
                at = escaped_codepoint_end(text, at);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += try_skip_string(&text[at..], ch, admission)?;
                continue;
            }
            ')' => return Ok(at + ch.len_utf8()),
            _ => at += ch.len_utf8(),
        }
    }
    Ok(text.len())
}

/// Byte length of a string body, including its closing quote if present.
/// Escapes hide delimiters; a bad string ends before its unescaped newline.
pub(crate) fn skip_string(text: &str, quote: char) -> usize {
    match scan_string(text, quote) {
        Ok(end) | Err(end) => end,
    }
}

pub(crate) fn try_skip_string(
    text: &str,
    quote: char,
    admission: &Admission,
) -> Result<usize, Error> {
    Ok(match try_scan_string(text, quote, admission)? {
        Ok(end) | Err(end) => end,
    })
}

/// Граница строки: Ok после кавычки/EOF, Err перед bad-string newline.
pub(crate) fn scan_string(text: &str, quote: char) -> Result<usize, usize> {
    try_scan_string(text, quote, &Admission::default()).expect("unlimited token scan")
}

fn try_scan_string(
    text: &str,
    quote: char,
    admission: &Admission,
) -> Result<Result<usize, usize>, Error> {
    let mut at = 0;
    while at < text.len() {
        admission.work(1)?;
        let ch = text[at..].chars().next().unwrap();
        if ch == '\\' {
            // At most six hex digits and one whitespace/CRLF terminator.
            admission.work(9.min(text.len() - at))?;
            // A hexadecimal escape consumes its terminating CSS whitespace,
            // including a newline; that newline cannot end a bad-string token.
            at = escape_end(text, at);
            continue;
        }
        if ch == quote {
            return Ok(Ok(at + ch.len_utf8()));
        }
        if matches!(ch, '\n' | '\r' | '\u{c}') {
            return Ok(Err(at));
        }
        at += ch.len_utf8();
    }
    Ok(Ok(text.len()))
}

pub(crate) fn escape_end(raw: &str, at: usize) -> usize {
    let mut end = at + 1;
    let mut digits = 0;
    while end < raw.len() && digits < 6 && raw.as_bytes()[end].is_ascii_hexdigit() {
        end += 1;
        digits += 1;
    }
    if let Some(c) = raw[end..].chars().next()
        && (digits == 0 || c.is_ascii_whitespace())
    {
        end += c.len_utf8();
        if c == '\r' && raw[end..].starts_with('\n') {
            end += 1;
        }
    }
    end
}

/// Пропуск обратного слеша и одного знака для поиска разделителей.
/// В отличие от escape_end не поглощает hex-последовательность и её пробел.
pub(crate) fn escaped_codepoint_end(text: &str, at: usize) -> usize {
    let body = at + 1;
    body + text[body..].chars().next().map_or(0, char::len_utf8)
}
