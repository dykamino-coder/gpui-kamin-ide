//! Вычисленный стиль: разбор аргументов `image()` (css-images-4 §2.2) — адрес
//! (строка или `url()`) и/или цвет. Здесь только синтаксис: какой аргумент
//! действует (запасной цвет при незагрузившемся адресе), решает загрузчик образа.

use crate::document::state::ValueCx;
use crate::style::{css, values::value::Color};

pub(crate) struct Arguments<'a> {
    pub url: Option<String>,
    pub color: Option<&'a str>,
}

pub(crate) fn arguments<'a>(raw: &'a str, vcx: ValueCx<'_>) -> Option<Arguments<'a>> {
    let raw = raw.trim();
    if !raw.get(..6)?.eq_ignore_ascii_case("image(") || !raw.ends_with(')') {
        return None;
    }
    let parts = css::split_args(&raw[6..raw.len() - 1]);
    let first = parts.first()?.trim();
    if parts.len() == 1 && is_color(first, vcx) {
        return Some(Arguments {
            url: None,
            color: Some(first),
        });
    }
    let url = source(first)?;
    let color = match parts.as_slice() {
        [_] => None,
        [_, color] if is_color(color.trim(), vcx) => Some(color.trim()),
        _ => return None,
    };
    Some(Arguments {
        url: Some(url),
        color,
    })
}

fn is_color(raw: &str, vcx: ValueCx<'_>) -> bool {
    raw.eq_ignore_ascii_case("currentcolor") || Color::parse(raw, vcx).is_some()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_arguments_preserve_fallback_until_loading() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let image = arguments(r#"image("missing.png", currentColor)"#, vcx).unwrap();
        assert_eq!(image.url.as_deref(), Some("missing.png"));
        assert_eq!(image.color, Some("currentColor"));
        assert!(arguments("image(red)", vcx).unwrap().url.is_none());
        assert!(arguments("image(url(a.png))", vcx).unwrap().color.is_none());
        for raw in [
            "image()",
            "image(red, blue)",
            r#"image("a", "b")"#,
            r#"image("a" trailing, red)"#,
            "image(url(a), red, blue)",
        ] {
            assert!(arguments(raw, vcx).is_none(), "{raw}");
        }
    }
}
