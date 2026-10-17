//! CSS image() URL and color arguments; fallback is resolved by the image loader.

use crate::style::{css, values::value::Color};

pub(crate) struct Arguments<'a> {
    pub url: Option<String>,
    pub raw_url: Option<&'a str>,
    pub color: Option<&'a str>,
}

pub(crate) fn arguments(raw: &str) -> Option<Arguments<'_>> {
    let raw = raw.trim();
    if !raw.get(..6)?.eq_ignore_ascii_case("image(") || !raw.ends_with(')') {
        return None;
    }
    let parts = css::split_args(&raw[6..raw.len() - 1]);
    let first = parts.first()?.trim();
    if parts.len() == 1 && is_color(first) {
        return Some(Arguments {
            url: None,
            raw_url: None,
            color: Some(first),
        });
    }
    let url = source(first)?;
    let color = match parts.as_slice() {
        [_] => None,
        [_, color] if is_color(color.trim()) => Some(color.trim()),
        _ => return None,
    };
    Some(Arguments {
        url: Some(url),
        raw_url: Some(first),
        color,
    })
}

fn is_color(raw: &str) -> bool {
    raw.eq_ignore_ascii_case("currentcolor") || Color::parse(raw).is_some()
}

fn source(raw: &str) -> Option<String> {
    match raw.chars().next()? {
        q @ ('\'' | '"') => {
            if !raw.ends_with(q)
                || raw.len() < 2
                || css::skip_string(&raw[1..], q) + 1 != raw.len()
                || raw[..raw.len() - 1]
                    .chars()
                    .rev()
                    .take_while(|c| *c == '\\')
                    .count()
                    % 2
                    != 0
            {
                return None;
            }
            Some(css::unescape(&raw[1..raw.len() - 1]))
        }
        _ if css::at_url(raw) && css::skip_url(raw) == raw.len() => super::parse_url(raw),
        _ => None,
    }
}

pub(crate) fn color_only(raw: &str) -> Option<&str> {
    let args = arguments(raw)?;
    args.url.is_none().then_some(args.color).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_arguments_preserve_fallback_until_loading() {
        let image = arguments(r#"image("missing.png", currentColor)"#).unwrap();
        assert_eq!(image.url.as_deref(), Some("missing.png"));
        assert_eq!(image.color, Some("currentColor"));
        assert!(arguments("image(red)").unwrap().url.is_none());
        assert!(arguments("image(url(a.png))").unwrap().color.is_none());
        for raw in [
            "image()",
            "image(red, blue)",
            r#"image("a", "b")"#,
            r#"image("a" trailing, red)"#,
            "image(url(a), red, blue)",
        ] {
            assert!(arguments(raw).is_none(), "{raw}");
        }
    }
}
