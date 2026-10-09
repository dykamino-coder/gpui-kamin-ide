//! Parse list shorthand components before mutating the cascaded style.
//! CSS Lists 3 §3.6: none fills unset image/type components, not every type.
use super::{Computed, list_style_string, split_outside_parens};

pub(super) fn apply(style: &mut Computed, key: &str, value: &str) {
    if list_style_string::apply_string(style, key, value) {
        style.list_style_type = None;
        return;
    }
    let words = split_outside_parens(value);
    let (mut kind, mut position, mut image, mut none) = (None, None, false, 0);
    for word in &words {
        let decoded = crate::style::css::unescape(word);
        let keyword = decoded.to_ascii_lowercase();
        if key == "list-style" && matches!(keyword.as_str(), "inside" | "outside") {
            if position.replace(keyword == "inside").is_some() {
                return;
            }
        } else if keyword == "none" {
            none += 1;
        } else if key == "list-style" && is_image(&keyword) {
            if image {
                return;
            }
            image = true;
        } else if crate::style::css::selector_tokens::ident(word) || keyword.starts_with("symbols(") {
            // css-counter-styles-3 §symbols-function: an invalid anonymous
            // style invalidates the whole declaration.
            if keyword.starts_with("symbols(")
                && !crate::style::generated::counter_style_rules::valid_symbols_fn(word)
            {
                return;
            }
            if matches!(
                keyword.as_str(),
                "inherit" | "initial" | "unset" | "revert" | "revert-layer" | "default"
            ) || kind.replace(decoded).is_some()
            {
                return;
            }
        } else {
            return;
        }
    }
    if words.is_empty() || (key == "list-style-type" && words.len() != 1) {
        return;
    }
    // An extra none cannot replace a component already specified. In
    // particular `none square` sets an image, while `none none square`
    // invalidates the declaration and preserves the earlier disc.
    let slots = usize::from(kind.is_none()) + usize::from(!image);
    if none > slots {
        return;
    }
    if none > 0 && kind.is_none() {
        kind = Some("none".into());
    }
    style.no_marker = Some(kind.as_deref() == Some("none"));
    style.marker_text = None;
    style.list_style_type = Some(kind.unwrap_or_else(|| "disc".into()));
    if key == "list-style" {
        style.list_style_inside = Some(position.unwrap_or(false));
    }
}

fn is_image(value: &str) -> bool {
    value.starts_with("url(")
        || value.contains("-gradient(")
        || ["image(", "image-set(", "cross-fade("]
            .iter()
            .any(|name| value.starts_with(name))
}
