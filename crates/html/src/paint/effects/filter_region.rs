//! Filter region of a `<filter>` referenced by an HTML box (`filter: url(#id)`).
//!
//! Units follow SVG 2 §8.11 / Filter Effects 1 §5: with `userSpaceOnUse` the
//! user space is the box (origin at its corner, percentages of its size); with
//! `objectBoundingBox` (default) a number or percentage is a fraction of the box
//! and a length with units (`1em`, `0.25px`) is first taken in user units and
//! then read as a fraction too (usvg `units::convert_length` does the same).

use std::borrow::Cow;

/// Attribute value inside the `<filter` start tag.
fn attr<'a>(open: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let at = open.find(&key)? + key.len();
    let rest = &open[at..];
    Some(rest[..rest.find('"')?].trim())
}

/// The `<filter …>` start tag of the definition.
fn open_tag(def: &str) -> Option<(usize, &str)> {
    let at = def.find("<filter")?;
    let open = &def[at..];
    Some((at, &open[..open.find('>').unwrap_or(open.len())]))
}

/// One region length in points from the box corner (`base` — box side).
/// `calc()` sums its `+`/`-` terms term by term (css-values-4 §10).
fn length(v: &str, base: f32, user: bool, font: f32) -> Option<f32> {
    let v = v.trim();
    if let Some(inner) = v.strip_prefix("calc(").and_then(|r| r.strip_suffix(')')) {
        let mut sum = 0.0;
        let mut sign = 1.0;
        for tok in inner.split_whitespace() {
            match tok {
                "+" => sign = 1.0,
                "-" => sign = -1.0,
                t => sum += sign * length(t, base, user, font)?,
            }
        }
        return Some(sum);
    }
    let (n, frac) = if let Some(p) = v.strip_suffix('%') {
        (p.trim().parse::<f32>().ok()? / 100.0, true)
    } else if let Some(e) = v.strip_suffix("em") {
        (e.trim().parse::<f32>().ok()? * font, false)
    } else {
        (v.trim_end_matches("px").trim().parse::<f32>().ok()?, false)
    };
    Some(if frac || !user { n * base } else { n })
}

fn font_size(open: &str) -> f32 {
    attr(open, "font-size")
        .and_then(|v| v.trim_end_matches("px").trim().parse::<f32>().ok())
        .unwrap_or(16.0)
}

const REGION: [(&str, bool); 4] = [
    ("x", true),
    ("y", false),
    ("width", true),
    ("height", false),
];

/// Region `[x, y, width, height]` in points from the box corner; a missing
/// attribute takes the default −10 %/120 %.
pub(super) fn filter_region(def: &str, w: f32, h: f32) -> [f32; 4] {
    let defaults = [-0.1 * w, -0.1 * h, 1.2 * w, 1.2 * h];
    let Some((_, open)) = open_tag(def) else {
        return defaults;
    };
    let user = open.contains("userSpaceOnUse");
    let font = font_size(open);
    let mut out = defaults;
    for (i, (name, horizontal)) in REGION.iter().enumerate() {
        let base = if *horizontal { w } else { h };
        if let Some(v) = attr(open, name).and_then(|v| length(v, base, user, font)) {
            out[i] = v;
        }
    }
    out
}

/// The definition with `calc()` region attributes replaced by plain numbers
/// in the same unit system (a fraction for `objectBoundingBox`, user units
/// for `userSpaceOnUse`): usvg does not parse `calc()` and dropped the region.
pub(super) fn resolve_calc(def: &str, w: f32, h: f32) -> Cow<'_, str> {
    let Some((at, open)) = open_tag(def) else {
        return Cow::Borrowed(def);
    };
    if !open.contains("calc(") {
        return Cow::Borrowed(def);
    }
    let user = open.contains("userSpaceOnUse");
    let font = font_size(open);
    let mut tag = open.to_string();
    for (name, horizontal) in REGION {
        let Some(v) = attr(open, name).filter(|v| v.starts_with("calc(")) else {
            continue;
        };
        let base = if horizontal { w } else { h };
        let Some(px) = length(v, base, user, font) else {
            continue;
        };
        let plain = if user || base <= 0.0 { px } else { px / base };
        tag = tag.replacen(
            &format!(" {name}=\"{v}\""),
            &format!(" {name}=\"{plain}\""),
            1,
        );
    }
    Cow::Owned(format!("{}{tag}{}", &def[..at], &def[at + open.len()..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounding_box_lengths_are_fractions() {
        let def = r#"<filter id="f" x="0" y="0" width="1em" height="1" font-size="10px">"#;
        assert_eq!(filter_region(def, 10.0, 100.0), [0.0, 0.0, 100.0, 100.0]);
    }

    #[test]
    fn calc_region_is_summed_and_rewritten() {
        let def = r#"<filter id="f" x="0" y="0" width="calc(25% + 0.25px)" height="1"></filter>"#;
        let out = resolve_calc(def, 200.0, 100.0);
        assert!(out.contains(r#"width="0.5""#), "{out}");
        assert_eq!(filter_region(&out, 200.0, 100.0), [0.0, 0.0, 100.0, 100.0]);
    }

    #[test]
    fn user_space_percentages_are_of_the_box() {
        let def = r#"<filter filterUnits="userSpaceOnUse" x="10" y="50%" width="20px" height="calc(50% - 10px)">"#;
        assert_eq!(filter_region(def, 100.0, 40.0), [10.0, 20.0, 20.0, 10.0]);
    }
}
