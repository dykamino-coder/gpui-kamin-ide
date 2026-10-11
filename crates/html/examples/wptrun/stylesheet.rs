//! Stylesheet text as a browser would fetch it: byte decoding, `url(...)` rebasing and lookup.

/// Прочитать подключённую таблицу стилей БАЙТАМИ, как это делает браузер
/// с wpt-сервером.
///
/// `read_to_string` молча съедал любой не-UTF-8 файл (`unwrap_or_default`
/// давал пустую таблицу — семь at-charset-тестов краснели одинаковыми 3.53).
/// Рядом с файлом может лежать `<имя>.headers` с HTTP-заголовками: не
/// `text/css` — таблица не подключается вовсе; `charset=` из заголовка
/// слабее метки порядка байтов, но сильнее `@charset` в самом файле.
pub(super) fn read_stylesheet(file: &std::path::Path) -> String {
    let Ok(bytes) = std::fs::read(file) else {
        return String::new();
    };
    let headers = std::fs::read_to_string(format!("{}.headers", file.display())).ok();
    let content_type = headers.as_deref().and_then(|h| {
        h.lines()
            .find(|l| l.to_ascii_lowercase().starts_with("content-type:"))
            .map(|l| l[13..].trim().to_string())
    });
    if let Some(ct) = &content_type
        && !ct.to_ascii_lowercase().starts_with("text/css")
    {
        return String::new();
    }
    let at_charset = || {
        let head = bytes.get(..bytes.len().min(64))?;
        let text = head.strip_prefix(b"@charset \x22")?;
        let end = text.iter().position(|b| *b == b'\x22')?;
        encoding_rs::Encoding::for_label(&text[..end])
    };
    let enc = kamin_html::encoding::from_bom(&bytes)
        .or_else(|| {
            content_type
                .as_deref()
                .and_then(kamin_html::encoding::from_content_type)
        })
        .or_else(at_charset)
        .map(kamin_html::encoding::fix_utf16)
        .unwrap_or(encoding_rs::UTF_8);
    enc.decode(&bytes).0.into_owned()
}

/// Перебазировать `url(...)` содержимого css-файла на его папку: после
/// инлайна в документ относительные адреса считались бы от папки ТЕСТА.
pub(super) fn rebase_css_urls(css: &str, base: &std::path::Path) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some((at, head)) = find_url(rest) {
        out.push_str(&rest[..at + head]);
        rest = &rest[at + head..];
        let Some(end) = rest.find(')') else { break };
        let raw = rest[..end].trim();
        let bare = raw.trim_matches(|c| c == '\'' || c == '"');
        if bare.starts_with("data:") || bare.contains("://") || bare.starts_with('/') {
            out.push_str(raw);
        } else {
            let abs = base.join(bare).display().to_string().replace('\\', "/");
            out.push('"');
            out.push_str(&abs);
            out.push('"');
        }
        out.push(')');
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Ближайшая запись `url(` без учёта регистра, включая экранированные формы
/// имени (`U\\r\\4c (` — то же `url(`, §4.1.3): даёт начало и длину головы
/// вместе с открывающей скобкой.
pub(super) fn find_url(text: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    (0..bytes.len()).find_map(|i| {
        if !(bytes[i] == b'\\' || bytes[i].eq_ignore_ascii_case(&b'u')) {
            return None;
        }
        url_head(&text[i..]).map(|len| (i, len))
    })
}

/// Длина головы `url(` (до скобки включительно), если запись начинается здесь.
///
/// Имя записи может нести экранирование: hex-код с необязательным
/// пробелом-терминатором либо один буквальный знак. Пробел-терминатор — часть
/// эскейпа, отдельных пробелов между именем и скобкой не бывает.
fn url_head(text: &str) -> Option<usize> {
    let mut at = 0usize;
    let mut name = String::new();
    while at < text.len() && name.len() < 3 {
        let ch = text[at..].chars().next()?;
        match ch {
            '\\' => {
                let tail = &text[at + 1..];
                let mut digits = 0usize;
                let mut end = 0usize;
                for (i, c) in tail.char_indices() {
                    if digits < 6 && c.is_ascii_hexdigit() {
                        digits += 1;
                        end = i + c.len_utf8();
                        continue;
                    }
                    if digits > 0 && c.is_whitespace() {
                        end = i + c.len_utf8();
                    }
                    break;
                }
                if digits > 0 {
                    let code = u32::from_str_radix(tail[..end].trim(), 16).ok()?;
                    name.push(char::from_u32(code)?);
                    at += 1 + end;
                } else {
                    let c = tail.chars().next()?;
                    name.push(c);
                    at += 1 + c.len_utf8();
                }
            }
            c if c.is_ascii_alphabetic() => {
                name.push(c);
                at += c.len_utf8();
            }
            _ => return None,
        }
    }
    if !name.eq_ignore_ascii_case("url") {
        return None;
    }
    (text.as_bytes().get(at) == Some(&b'(')).then_some(at + 1)
}
