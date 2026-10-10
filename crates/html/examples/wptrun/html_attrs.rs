//! Tag attribute lookup, stylesheet `<link>` detection and percent-decoding of URLs.

/// `<link>` подключает таблицу стилей — по атрибуту `rel`, а не по тексту
/// тега: имя файла эталона `layer-stylesheet-sharing-ref.html` содержит слово
/// «stylesheet», и `<link rel="match">` разворачивался в таблицу —
/// эталон вклеивался прямо в тест.
pub(super) fn is_stylesheet_link(tag: &str) -> bool {
    // `alternate stylesheet` по умолчанию не применяется (HTML §4.6.7.1:
    // «alternative style sheet … not applied by default»).
    attr_value(tag, "rel").is_some_and(|rel| {
        let mut toks = rel.split_ascii_whitespace();
        toks.clone().any(|t| t.eq_ignore_ascii_case("stylesheet"))
            && !toks.any(|t| t.eq_ignore_ascii_case("alternate"))
    })
}

/// Адрес с раскодированными процентами.
pub(super) fn percent_decode(raw: &str) -> String {
    if !raw.contains('%') {
        return raw.to_string();
    }
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut at = 0usize;
    while at < bytes.len() {
        if bytes[at] == b'%' && at + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[at + 1..at + 3]).unwrap_or("");
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                at += 3;
                continue;
            }
        }
        out.push(bytes[at]);
        at += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| raw.to_string())
}

/// Значение атрибута в теге — в кавычках или без.
///
/// Разбор идёт с учётом кавычек: имя атрибута ищется только ВНЕ значений,
/// иначе `alt="см. src=1"` подсовывает чужое значение.
pub(super) fn attr_value(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut i = 0usize;
    let mut quote: Option<u8> = None;
    let mut word_start: Option<usize> = None;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' | b'\'' => {
                quote = Some(c);
                word_start = None;
            }
            b'=' => {
                if let Some(start) = word_start.take()
                    && tag[start..i].trim().eq_ignore_ascii_case(name)
                {
                    let value = tag[i + 1..].trim_start();
                    let quoted = |q: char| value.strip_prefix(q).and_then(|v| v.split(q).next());
                    return quoted('"')
                        .or_else(|| quoted('\''))
                        .or_else(|| value.split([' ', '>', '/']).next())
                        .map(str::to_string);
                }
            }
            c if c.is_ascii_whitespace() => word_start = None,
            _ => {
                if word_start.is_none() {
                    word_start = Some(i);
                }
            }
        }
        i += 1;
    }
    None
}
