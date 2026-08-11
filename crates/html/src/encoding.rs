//! Определение кодировки байтового потока разметки.
//!
//! Разбор дерева принимает готовую строку, а из сети и с диска приходят
//! БАЙТЫ, и какими они прочитаны — сказано внутри самих байтов: меткой
//! порядка байтов или `<meta charset>` в начале документа. Пока эта ступень
//! отсутствовала, байты приходилось читать наугад, и страница в
//! `windows-1251` превращалась в вопросительные знаки.
//!
//! Алгоритм — «предпросмотр потока» из спецификации: разметка проходится
//! упрощённым разбором, чтобы `charset` в комментарии, в значении чужого
//! атрибута или в незакрытом теге не был принят за объявление кодировки.

use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};

/// Кодировка потока: метка порядка байтов, затем `<meta>`, иначе умолчание.
///
/// Умолчание — `windows-1252`: так поступает браузер, когда о кодировке не
/// сказано ничего.
pub fn sniff(bytes: &[u8]) -> &'static Encoding {
    if let Some(bom) = from_bom(bytes) {
        return bom;
    }
    prescan(bytes).unwrap_or(WINDOWS_1252)
}

/// Метка порядка байтов сильнее всего остального.
pub fn from_bom(bytes: &[u8]) -> Option<&'static Encoding> {
    match bytes {
        [0xEF, 0xBB, 0xBF, ..] => Some(UTF_8),
        [0xFE, 0xFF, ..] => Some(encoding_rs::UTF_16BE),
        [0xFF, 0xFE, ..] => Some(encoding_rs::UTF_16LE),
        _ => None,
    }
}

/// Предпросмотр разметки ради объявления кодировки.
///
/// Просматривается весь поток, а не первая тысяча байт: браузер, встретив
/// объявление позже, перечитывает документ заново, и итог тот же — но без
/// второго прохода.
fn prescan(bytes: &[u8]) -> Option<&'static Encoding> {
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"<!--") {
            // Комментарий пропускается целиком: `charset` внутри не в счёт.
            i = find(bytes, b"-->", i + 4)? + 3;
            continue;
        }
        if starts_with_ignore_case(&bytes[i..], b"<meta")
            && bytes
                .get(i + 5)
                .is_some_and(|c| c.is_ascii_whitespace() || *c == b'/')
        {
            let mut cursor = i + 5;
            let mut seen: Vec<String> = vec![];
            let (mut charset, mut equiv_is_content_type, mut content) = (None, false, None);
            while let Some((name, value)) = attribute(bytes, &mut cursor) {
                // Повторное имя не рассматривается — так велит алгоритм.
                if seen.contains(&name) {
                    continue;
                }
                match name.as_str() {
                    "charset" => charset = Encoding::for_label(value.trim().as_bytes()),
                    "http-equiv" => {
                        equiv_is_content_type = value.eq_ignore_ascii_case("content-type")
                    }
                    "content" => content = Some(value.clone()),
                    _ => {}
                }
                seen.push(name);
            }
            // Тег обязан закрыться: `<meta charset=euc-jp` без `>` браузер
            // объявлением не считает.
            if cursor >= bytes.len() && bytes.last() != Some(&b'>') {
                return None;
            }
            if let Some(found) = charset {
                return Some(fix_utf16(found));
            }
            if equiv_is_content_type
                && let Some(text) = content
                && let Some(found) = from_content_type(&text)
            {
                return Some(fix_utf16(found));
            }
            i = cursor;
            continue;
        }
        // Прочий тег: его атрибуты пропускаются тем же разбором, иначе
        // `charset=` внутри чужого значения принимался бы за объявление.
        if bytes[i] == b'<'
            && bytes
                .get(i + 1)
                .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'/')
        {
            let mut cursor = i + 1;
            while cursor < bytes.len()
                && !bytes[cursor].is_ascii_whitespace()
                && bytes[cursor] != b'>'
            {
                cursor += 1;
            }
            while attribute(bytes, &mut cursor).is_some() {}
            // Незакрытый тег — конец потока: объявления в нём уже не будет.
            if cursor >= bytes.len() {
                return None;
            }
            i = cursor;
            continue;
        }
        if bytes[i] == b'<' && matches!(bytes.get(i + 1), Some(b'!') | Some(b'/') | Some(b'?')) {
            i = find(bytes, b">", i + 2)? + 1;
            continue;
        }
        i += 1;
    }
    None
}

/// Метка кодировки из значения `content` у `http-equiv=content-type`.
fn from_content_type(text: &str) -> Option<&'static Encoding> {
    let lower = text.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find("charset") {
        let start = from + found + "charset".len();
        let rest = text[start..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            from = start;
            continue;
        };
        let rest = rest.trim_start();
        let label: String = match rest.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                // Кавычка обязана закрыться: `charset='utf-8` без пары
                // объявлением не считается.
                let inner = &rest[1..];
                let end = inner.find(quote)?;
                inner[..end].to_string()
            }
            _ => rest
                .chars()
                .take_while(|c| !c.is_ascii_whitespace() && *c != ';')
                .collect(),
        };
        return Encoding::for_label(label.trim().as_bytes());
    }
    None
}

/// Очередной атрибут тега — разбор из спецификации.
///
/// Возвращает имя в нижнем регистре и значение; `None` — атрибуты кончились
/// (встретился `>` или поток).
fn attribute(bytes: &[u8], cursor: &mut usize) -> Option<(String, String)> {
    let mut i = *cursor;
    while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
        i += 1;
    }
    if i >= bytes.len() {
        *cursor = i;
        return None;
    }
    if bytes[i] == b'>' {
        *cursor = i + 1;
        return None;
    }
    let mut name = String::new();
    while i < bytes.len() {
        match bytes[i] {
            b'=' if !name.is_empty() => break,
            b'>' | b'/' => break,
            c if c.is_ascii_whitespace() => break,
            c => name.push(c.to_ascii_lowercase() as char),
        }
        i += 1;
    }
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'=' {
        // Атрибут без значения.
        *cursor = i;
        return Some((name, String::new()));
    }
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let mut value = String::new();
    if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
        let quote = bytes[i];
        i += 1;
        while i < bytes.len() && bytes[i] != quote {
            value.push(bytes[i] as char);
            i += 1;
        }
        // Незакрытая кавычка — конец потока.
        if i >= bytes.len() {
            *cursor = i;
            return None;
        }
        i += 1;
    } else {
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
            value.push(bytes[i] as char);
            i += 1;
        }
    }
    *cursor = i;
    Some((name, value))
}

/// Двухбайтовые кодировки в предпросмотре заменяются на UTF-8.
///
/// Так велит спецификация: метка `utf-16` внутри самого документа значила бы,
/// что документ не мог быть прочитан однобайтовым предпросмотром вовсе.
fn fix_utf16(found: &'static Encoding) -> &'static Encoding {
    match found {
        e if e == encoding_rs::UTF_16LE || e == encoding_rs::UTF_16BE => UTF_8,
        e if e == encoding_rs::X_USER_DEFINED => WINDOWS_1252,
        other => other,
    }
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|p| p + from)
}

fn starts_with_ignore_case(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.len() >= needle.len() && haystack[..needle.len()].eq_ignore_ascii_case(needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_wins_over_meta() {
        assert_eq!(sniff(b"\xEF\xBB\xBF<meta charset=windows-1251>"), UTF_8);
    }

    #[test]
    fn meta_charset_is_read() {
        assert_eq!(
            sniff(b"<!DOCTYPE html><meta charset='iso8859-2'>"),
            encoding_rs::ISO_8859_2
        );
    }

    #[test]
    fn charset_inside_comment_is_ignored() {
        assert_eq!(
            sniff(b"<!-- <meta charset=iso8859-2> --><meta charset=utf-8>"),
            UTF_8
        );
    }

    #[test]
    fn charset_inside_other_attribute_is_ignored() {
        assert_eq!(
            sniff(b"<meta test=\" charset=iso8859-2>\n<p>\"</p>"),
            WINDOWS_1252
        );
    }

    #[test]
    fn unterminated_tag_gives_nothing() {
        assert_eq!(sniff(b"<meta charset=euc-jp"), WINDOWS_1252);
    }

    #[test]
    fn default_is_windows_1252() {
        assert_eq!(sniff(b"<!DOCTYPE html><p>text"), WINDOWS_1252);
    }
}
