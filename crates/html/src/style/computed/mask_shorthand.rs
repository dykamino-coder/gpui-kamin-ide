//! Expand a single mask layer into the existing mask longhand representation.

use super::{Computed, split_outside_parens};
use crate::style::values::value::Len;

pub(super) fn apply(style: &mut Computed, value: &str) -> bool {
    // Layer-specific size, origin, clip and mode lists need their own computed
    // representation. Keep the existing multi-layer route until it has one.
    if crate::style::css::split_args(value).len() != 1 {
        return false;
    }
    let mut spaced = String::new();
    let mut depth = 0usize;
    for ch in value.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '/' if depth == 0 => {
                spaced.push_str(" / ");
                continue;
            }
            _ => {}
        }
        spaced.push(ch);
    }
    let words = split_outside_parens(&spaced);
    let (mut image, mut mode, mut composite) = (None, None, None);
    let (mut position, mut size, mut repeats, mut boxes) = (vec![], vec![], vec![], vec![]);
    let (mut after_slash, mut no_clip) = (false, false);
    for word in &words {
        let t = word.as_str();
        if t == "none" || t.starts_with("url(") || t.contains("-gradient(") {
            if image.replace(t).is_some() {
                return false;
            }
        } else if matches!(t, "alpha" | "luminance" | "match-source") {
            if mode.replace(t).is_some() {
                return false;
            }
        } else if matches!(t, "add" | "subtract" | "intersect" | "exclude") {
            if composite.replace(t).is_some() {
                return false;
            }
        } else if matches!(
            t,
            "repeat" | "no-repeat" | "repeat-x" | "repeat-y" | "space" | "round"
        ) {
            repeats.push(t);
        } else if matches!(
            t,
            "border-box" | "padding-box" | "content-box" | "fill-box" | "stroke-box" | "view-box"
        ) {
            boxes.push(t);
        } else if t == "no-clip" {
            if no_clip {
                return false;
            }
            no_clip = true;
        } else if t == "/" {
            if after_slash || position.is_empty() {
                return false;
            }
            after_slash = true;
        } else if after_slash
            && (matches!(t, "auto" | "contain" | "cover") || Len::parse(t).is_some())
        {
            size.push(t);
        } else if !after_slash
            && (matches!(t, "left" | "right" | "top" | "bottom" | "center")
                || Len::parse(t).is_some())
        {
            position.push(t);
        } else {
            return false;
        }
    }
    if boxes.len() > 2
        || (no_clip && boxes.len() > 1)
        || repeats.len() > 2
        || position.len() > 4
        || position.len() == 3
        || size.len() > 2
        || (after_slash && size.is_empty())
        || (size.len() == 2 && size.iter().any(|s| matches!(*s, "contain" | "cover")))
    {
        return false;
    }
    if position.len() == 2 {
        if (matches!(position[0], "top" | "bottom")
            && matches!(position[1], "left" | "right" | "center"))
            || (position[0] == "center" && matches!(position[1], "left" | "right"))
        {
            position.swap(0, 1);
        }
        if matches!(position[0], "top" | "bottom") || matches!(position[1], "left" | "right") {
            return false;
        }
    }
    if position.len() == 4 {
        let horizontal = |t| matches!(t, "left" | "right");
        let vertical = |t| matches!(t, "top" | "bottom");
        if !((horizontal(position[0]) && vertical(position[2]))
            || (vertical(position[0]) && horizontal(position[2])))
            || Len::parse(position[1]).is_none()
            || Len::parse(position[3]).is_none()
        {
            return false;
        }
    }
    if size.iter().any(|s| s.starts_with('-'))
        || (repeats.len() == 2
            && repeats
                .iter()
                .any(|s| matches!(*s, "repeat-x" | "repeat-y")))
    {
        return false;
    }
    let origin = boxes.first().copied().unwrap_or("border-box");
    let clip = if no_clip {
        "no-clip"
    } else {
        boxes.get(1).copied().unwrap_or(origin)
    };
    // CSS Masking section 7.9: omitted shorthand components reset to their initial
    // values. Store only the image token, not its position or slash-size tail.
    style.mask_image = image.filter(|i| *i != "none").map(str::to_owned);
    for (key, value) in [
        (
            "mask-position",
            if position.is_empty() {
                "0% 0%".to_string()
            } else {
                position.join(" ")
            },
        ),
        (
            "mask-size",
            if size.is_empty() {
                "auto auto".to_string()
            } else {
                size.join(" ")
            },
        ),
        (
            "mask-repeat",
            if repeats.is_empty() {
                "repeat".to_string()
            } else {
                repeats.join(" ")
            },
        ),
        ("mask-origin", origin.to_string()),
        ("mask-clip", clip.to_string()),
        ("mask-mode", mode.unwrap_or("match-source").to_string()),
        ("mask-composite", composite.unwrap_or("add").to_string()),
    ] {
        style.apply_one(key, &value);
    }
    true
}
