//! Загрузка локальных CSS: URL остаются относительно исходной таблицы.

use std::path::PathBuf;
use url::Url;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(super) fn file(url: &Url) -> Result<PathBuf> {
    if url.scheme() != "file" {
        return Err(format!("network resource is unsupported: {url}").into());
    }
    let path = url.to_file_path().map_err(|_| "invalid local file URL")?;
    if !path.is_file() {
        return Err(format!("missing local resource: {}", path.display()).into());
    }
    Ok(path)
}

pub(super) fn load(url: &Url, stack: &mut Vec<Url>) -> Result<String> {
    if stack.contains(url) || stack.len() >= 32 {
        return Err(format!("cyclic or excessively deep CSS import: {url}").into());
    }
    let bytes = std::fs::read(file(url)?)?;
    let charset = bytes.strip_prefix(b"@charset \"").and_then(|rest| {
        let end = rest.iter().position(|c| *c == b'"')?;
        encoding_rs::Encoding::for_label(&rest[..end])
    });
    let encoding = kamin_html::encoding::from_bom(&bytes)
        .or(charset)
        .unwrap_or(encoding_rs::UTF_8);
    let text = encoding.decode(&bytes).0;
    stack.push(url.clone());
    let result = expand(&text, url, stack);
    stack.pop();
    result
}

/// End of the string token opened at `start` (css-syntax-3 §4.3.5): the
/// closing quote, an unescaped newline (a bad-string, which ends there and
/// costs only its declaration) or the end of the sheet.
fn quoted_end(text: &str, start: usize) -> Result<usize> {
    let bytes = text.as_bytes();
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'\n' | b'\r' | b'\x0c' => return Ok(i),
            c if c == quote => return Ok(i + 1),
            _ => i += 1,
        }
    }
    Ok(bytes.len())
}

fn url_end(text: &str, start: usize) -> Result<usize> {
    let mut i = start;
    while i < text.len() {
        match text.as_bytes()[i] {
            b'\'' | b'"' => i = quoted_end(text, i)?,
            b'\\' => i += 2,
            b')' => return Ok(i),
            _ => i += 1,
        }
    }
    Err("unterminated CSS url() in local stylesheet".into())
}

/// Bytes an escape at `i` takes outside strings (css-syntax-3 §4.3.7): a
/// backslash and the code point after it, so an escaped quote (`\'` in a
/// Tailwind class selector) opens no string.
fn escape_len(text: &str, i: usize) -> usize {
    1 + text[i + 1..].chars().next().map_or(0, char::len_utf8)
}

fn bare(text: &str) -> Result<String> {
    Ok(kamin_html::css::try_unescape(
        text.trim().trim_matches(['\'', '"']),
    )?)
}

pub(super) fn expand(text: &str, base: &Url, stack: &mut Vec<Url>) -> Result<String> {
    let mut out = String::with_capacity(text.len());
    let (mut i, mut copied) = (0, 0);
    while i < text.len() {
        let tail = &text[i..];
        if tail.starts_with("/*") {
            i += tail.find("*/").map_or(tail.len(), |n| n + 2);
        } else if tail.as_bytes()[0] == b'\\' {
            i += escape_len(text, i);
        } else if matches!(tail.as_bytes()[0], b'\'' | b'"') {
            i = quoted_end(text, i)?;
        } else if tail
            .get(..7)
            .is_some_and(|s| s.eq_ignore_ascii_case("@import"))
            && tail.as_bytes().get(7).is_none_or(|c| {
                !c.is_ascii_alphanumeric() && !matches!(c, b'_' | b'-' | b'\\' | 128..=255)
            })
        {
            let rest = tail[7..].trim_start();
            let start = text.len() - rest.len();
            let (href, end) = if rest.starts_with(['\'', '"']) {
                let end = quoted_end(text, start)?;
                (bare(&text[start..end])?, end)
            } else if rest
                .get(..4)
                .is_some_and(|s| s.eq_ignore_ascii_case("url("))
            {
                let end = url_end(text, start + 4)?;
                (bare(&text[start + 4..end])?, end + 1)
            } else {
                return Err("unsupported CSS @import syntax".into());
            };
            let stop = text[end..].find(';').map_or(text.len(), |n| end + n);
            let media = text[end..stop].trim();
            if media.to_ascii_lowercase().contains("layer")
                || media.to_ascii_lowercase().contains("supports(")
            {
                return Err(
                    "CSS @import layer/supports qualifiers are unsupported by this host".into(),
                );
            }
            let css = load(&base.join(&href)?, stack)?;
            out.push_str(&text[copied..i]);
            if media.is_empty() {
                out.push_str(&css);
            } else {
                out.push_str(&format!("@media {media} {{\n{css}\n}}"));
            }
            i = (stop + 1).min(text.len());
            copied = i;
        } else if tail
            .get(..4)
            .is_some_and(|s| s.eq_ignore_ascii_case("url("))
            && (i == 0
                || !matches!(text.as_bytes()[i - 1], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-'))
        {
            let end = url_end(text, i + 4)?;
            let href = bare(&text[i + 4..end])?;
            if !href.is_empty() && !href.starts_with('#') && !href.starts_with("data:") {
                let url = base.join(&href)?;
                file(&url)?;
                out.push_str(&text[copied..i]);
                out.push_str(&format!("url(\"{url}\")"));
                copied = end + 1;
            }
            i = end + 1;
        } else {
            i += tail.chars().next().map_or(1, char::len_utf8);
        }
    }
    out.push_str(&text[copied..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_strings_and_data_urls_remain_literal() {
        let text = "/* url(no) */ p {content:'url(no)';background:url('data:image/svg+xml,<svg>(x)</svg>')}";
        let base = Url::parse("file:///C:/example/main.css").unwrap();
        assert_eq!(expand(text, &base, &mut vec![]).unwrap(), text);
    }

    #[test]
    fn escaped_quotes_and_bad_strings_do_not_swallow_the_sheet() {
        let base = Url::parse("file:///C:/example/main.css").unwrap();
        for text in [
            r".\[\&_svg\:not\(\[class\*\=\'size-\'\]\)\]{width:1px} a{color:red}",
            r".list-image-\[url\(\'data\:x\'\)\]{list-style:none}",
            "a{content:'open\n} b{color:red}",
            "a{content:'open at the end",
        ] {
            assert_eq!(expand(text, &base, &mut vec![]).unwrap(), text);
        }
    }

    #[test]
    fn invalid_and_network_resources_fail_instead_of_becoming_empty_css() {
        let base = Url::parse("file:///C:/example/main.css").unwrap();
        for text in [
            "a{background:url(abc}",
            "@import 'https://example.test/style.css';",
        ] {
            assert!(expand(text, &base, &mut vec![]).is_err());
        }
    }
}
