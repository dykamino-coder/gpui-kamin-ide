//! Однократный разбор грамматики CSS-изображений в семантические операнды.

use super::*;

impl ImageSource {
    pub(crate) fn parse(raw: &str, vcx: ValueCx<'_>) -> Option<Self> {
        let mut image = Self::parse_inner(raw.trim(), vcx, 0)?;
        image.resolve_lengths(&|_| {}, vcx);
        Some(image)
    }

    fn parse_inner(raw: &str, vcx: ValueCx<'_>, depth: usize) -> Option<Self> {
        if depth > 128 {
            return None;
        }
        let nested = |raw: &str| Self::parse_inner(raw.trim(), vcx, depth + 1);
        if crate::style::css::at_url(raw)
            && raw.ends_with(')')
            && let Some(url) = crate::style::computed::parse_url(raw)
        {
            return Some(Self::Url(url));
        }
        if let Some(inner) = raw
            .strip_prefix("image-set(")
            .or_else(|| raw.strip_prefix("-webkit-image-set("))
        {
            return match crate::style::computed::image_set_pick(inner.strip_suffix(')')?, vcx)? {
                Some(raw) => nested(&raw),
                None => Some(Self::Invalid),
            };
        }
        if let Some(inner) = raw
            .strip_prefix("light-dark(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let parts = crate::style::css::split_args(inner);
            if parts.len() != 2 {
                return None;
            }
            return nested(parts[usize::from(crate::style::values::value::dark_scheme(vcx))]);
        }
        if let Some(image) = crate::style::computed::image_function::arguments(raw, vcx) {
            let color = image.color.and_then(|c| ImageColor::parse(c, vcx));
            return match image.url {
                Some(url) => Some(Self::Fallback { url, color }),
                None => color.map(Self::Color),
            };
        }
        if let Some(inner) = raw
            .strip_prefix("filter(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let parts = crate::style::css::split_args(inner);
            if parts.len() < 2 {
                return None;
            }
            return Some(Self::Filter {
                image: Box::new(nested(parts[0])?),
                program: ImageFilter::parse(&parts[1..].join(" "), vcx),
            });
        }
        if let Some(inner) = raw
            .strip_prefix("cross-fade(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let mut items = Vec::new();
            for part in crate::style::css::split_args(inner) {
                let (mut weight, mut image) = (None, None);
                for word in crate::style::css::value_tokens::split_top(part) {
                    if let Some(pct) = word.strip_suffix('%').and_then(|n| n.parse::<f32>().ok()) {
                        weight = Some((pct / 100.0).clamp(0.0, 1.0));
                    } else {
                        image = Some(
                            nested(word)
                                .or_else(|| ImageColor::parse(word, vcx).map(Self::Color))?,
                        );
                    }
                }
                items.push((image?, weight));
            }
            if items.is_empty() {
                return None;
            }
            let given: f32 = items.iter().filter_map(|item| item.1).sum();
            let free = items.iter().filter(|item| item.1.is_none()).count() as f32;
            return Some(Self::CrossFade(
                items
                    .into_iter()
                    .map(|(image, weight)| {
                        let weight = match weight {
                            Some(p) if given > 1.0 => p / given,
                            Some(p) => p,
                            None if given >= 1.0 => 0.0,
                            None => (1.0 - given) / free,
                        };
                        (image, weight)
                    })
                    .collect(),
            ));
        }
        if raw
            .strip_prefix("repeating-")
            .unwrap_or(raw)
            .starts_with("conic-gradient(")
        {
            return ConicGradient::parse(raw, vcx).map(|g| Self::Conic(Box::new(g)));
        }
        crate::style::computed::parse_gradient(raw, vcx).map(|value| Self::Gradient {
            value: Box::new(value),
            repeating: raw.starts_with("repeating-"),
        })
    }
}
