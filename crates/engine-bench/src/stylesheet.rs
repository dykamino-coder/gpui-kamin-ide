//! Загрузка локальных CSS: URL остаются относительно исходной таблицы.

use std::{collections::BTreeSet, path::PathBuf, sync::Mutex};
use url::Url;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Resources the stand could not load in this process (a network URL or a missing local file).
static UNAVAILABLE: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// The unavailable resources seen so far, for the report: the page is measured as a browser shows it when
/// those fetches fail, and the report says how many there were.
pub(super) fn unavailable() -> Vec<String> {
    UNAVAILABLE
        .lock()
        .map(|set| set.iter().cloned().collect())
        .unwrap_or_default()
}

/// The local file of `url`, or `None` when it cannot be loaded here: the stand has no network and a page
/// snapshot may lack a file. That is a failed fetch, as in a browser (a stylesheet or an import is not applied,
/// an image is broken), so the page is still measured; the URL is recorded for the report.
pub(super) fn file(url: &Url) -> Result<Option<PathBuf>> {
    let path = match url.scheme() {
        "file" => url.to_file_path().map_err(|_| "invalid local file URL")?,
        _ => PathBuf::new(),
    };
    if !path.is_file() {
        if let Ok(mut set) = UNAVAILABLE.lock() {
            set.insert(url.to_string());
        }
        return Ok(None);
    }
    Ok(Some(path))
}

/// The expanded text of the stylesheet at `url`; `None` when it cannot be loaded (see `file`).
pub(super) fn load(url: &Url, stack: &mut Vec<Url>) -> Result<Option<String>> {
    if stack.contains(url) || stack.len() >= 32 {
        return Err(format!("cyclic or excessively deep CSS import: {url}").into());
    }
    let Some(path) = file(url)? else {
        return Ok(None);
    };
    let bytes = std::fs::read(path)?;
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
    result.map(Some)
}

fn quoted_end(text: &str, start: usize) -> usize {
    // CSS Syntax 3 §4.3.5: newline ends a bad string; EOF ends a string.
    // Preserve the source for the CSS parser, sharing its escape/token boundaries.
    start + 1 + kamin_html::css::skip_string(&text[start + 1..], text.as_bytes()[start] as char)
}

fn url_end(text: &str, start: usize) -> Result<usize> {
    let mut i = start;
    while i < text.len() {
        match text.as_bytes()[i] {
            b'\'' | b'"' => i = quoted_end(text, i),
            b'\\' => i += 2,
            b')' => return Ok(i),
            _ => i += 1,
        }
    }
    Err("unterminated CSS url() in local stylesheet".into())
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
        } else if let Some(escaped) = tail.strip_prefix('\\') {
            // An escaped quote in an identifier cannot open a string token.
            i += 1 + escaped.chars().next().map_or(0, char::len_utf8);
        } else if matches!(tail.as_bytes()[0], b'\'' | b'"') {
            i = quoted_end(text, i);
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
                let end = quoted_end(text, start);
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
            // A failed import is not applied: the rule contributes nothing.
            let css = load(&base.join(&href)?, stack)?.unwrap_or_default();
            out.push_str(&text[copied..i]);
            if media.is_empty() {
                out.push_str(&css);
            } else if !css.is_empty() {
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
                // an unavailable image or font stays referenced: its fetch fails as in a browser
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
    fn malformed_css_fails_instead_of_becoming_empty_css() {
        let base = Url::parse("file:///C:/example/main.css").unwrap();
        assert!(expand("a{background:url(abc}", &base, &mut vec![]).is_err());
    }

    #[test]
    fn unavailable_imports_are_dropped_and_recorded() {
        let base = Url::parse("file:///C:/example-missing/main.css").unwrap();
        let text = "@import 'https://example.test/style.css';@import 'gone.css' print;a{color:red}";
        assert_eq!(expand(text, &base, &mut vec![]).unwrap(), "a{color:red}");
        let seen = unavailable();
        assert!(seen.contains(&"https://example.test/style.css".to_string()));
        assert!(seen.contains(&"file:///C:/example-missing/gone.css".to_string()));
    }
    #[test]
    fn string_boundaries_share_css_escape_and_newline_recovery() {
        for newline in ["\n", "\r\n", "\u{c}"] {
            let text = format!("\"broken{newline}after");
            assert_eq!(quoted_end(&text, 0), "\"broken".len());
        }
        for text in [
            "\"unfinished",
            "\"unfinished\\",
            "\"escaped\\\r\nline\"",
            "\"hex\\a\nline\"",
            "\"utf8\\é\"",
        ] {
            assert_eq!(quoted_end(text, 0), text.len());
        }
    }
}
