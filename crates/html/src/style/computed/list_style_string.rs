//! String marker values are single CSS tokens even inside the list-style shorthand.
//! CSS Lists 3 §3.4 and §3.6: placement and the marker string are independent components.
use super::{Computed, collapse_segment_breaks, unescape_content};

pub(super) fn apply_string(style: &mut Computed, key: &str, raw: &str) -> bool {
    let mut rest = raw.trim();
    let mut marker = None;
    let mut placement = None;
    let mut image_none = false;
    while !rest.is_empty() {
        let ch = rest.chars().next().unwrap();
        if ch == '"' || ch == '\'' {
            if marker.is_some() {
                return false;
            }
            let len = crate::css::skip_string(&rest[1..], ch);
            let quoted = &rest[1..1 + len];
            if !quoted.ends_with(ch) {
                return false;
            }
            marker = Some(collapse_segment_breaks(&unescape_content(
                &quoted[..len - 1],
            )));
            rest = rest[1 + len..].trim_start();
        } else {
            if key != "list-style" {
                return false;
            }
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            match rest[..end].to_ascii_lowercase().as_str() {
                "inside" if placement.is_none() => placement = Some(true),
                "outside" if placement.is_none() => placement = Some(false),
                "none" if !image_none => image_none = true,
                _ => return false,
            }
            rest = rest[end..].trim_start();
        }
    }
    let Some(marker) = marker else {
        return false;
    };
    if key == "list-style" {
        style.list_style_inside = Some(placement.unwrap_or(false));
    }
    style.marker_text = Some(marker);
    style.no_marker = Some(false);
    true
}
