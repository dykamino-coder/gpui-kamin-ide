//! `unicode-range` of `@font-face` rules and the composite face of a family
//! (css-fonts-4 section 4.5): the rules with the selected descriptors form one
//! font, each serving only the characters of its own ranges, later rules first.

use super::FontRequest;
use super::matching::{Face, order, rank};

/// The code point ranges of a `unicode-range` descriptor. Empty means every
/// code point: no descriptor, or an invalid one, which is dropped as a whole.
/// Three forms (section 4.5): `U+20`, `U+0-7F` and the wildcard `U+00??`.
pub fn ranges(value: Option<&str>) -> Vec<(u32, u32)> {
    let Some(value) = value else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for part in value.split(',') {
        let part = part.trim();
        let Some(body) = part.strip_prefix("U+").or_else(|| part.strip_prefix("u+")) else {
            return Vec::new();
        };
        let hex = |v: &str| u32::from_str_radix(v.trim(), 16).ok();
        let (lo, hi) = match body.split_once('-') {
            Some((a, b)) => (hex(a), hex(b)),
            None if body.contains('?') => {
                (hex(&body.replace('?', "0")), hex(&body.replace('?', "F")))
            }
            None => (hex(body), hex(body)),
        };
        match (lo, hi) {
            (Some(lo), Some(hi)) if lo <= hi => out.push((lo, hi.min(0x10_FFFF))),
            _ => return Vec::new(),
        }
    }
    out
}

/// Whether `ranges` (see [`ranges`]) hold the code point `c`.
pub fn covers(ranges: &[(u32, u32)], c: u32) -> bool {
    ranges.is_empty() || ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&c))
}

/// The faces one request uses, in priority order. The best descriptors win by
/// section 5.2 (width, then style, then weight; equal ranks go to the later
/// rule); every rule with exactly those descriptors belongs to the composite,
/// later rules first. The first of them holding U+0020 leads: it is the first
/// available font, whose metrics set the line (section 5.4; Blink
/// `CSSSegmentedFontFace`, Gecko `gfxUserFontSet` group faces the same way).
pub fn composite<'a>(faces: &'a [Face], request: &FontRequest<'_>) -> Vec<&'a Face> {
    let Some(best) = faces
        .iter()
        .rev()
        .min_by(|a, b| order(&rank(a, request), &rank(b, request)))
    else {
        return Vec::new();
    };
    let same = |f: &Face| {
        f.width == best.width
            && f.weight == best.weight
            && f.slope == best.slope
            && f.angle == best.angle
    };
    let mut out: Vec<&Face> = faces.iter().rev().filter(|f| same(f)).collect();
    if let Some(i) = out.iter().position(|f| covers(&f.ranges, 0x20)) {
        let lead = out.remove(i);
        out.insert(0, lead);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(family: &str, weight: f32, ranges: Vec<(u32, u32)>) -> Face {
        Face {
            width: (100.0, 100.0),
            weight: (weight, weight),
            slope: 0,
            angle: (0.0, 0.0),
            family: family.into(),
            adjust: 1.0,
            features: Vec::new(),
            ranges,
        }
    }

    #[test]
    fn descriptor_forms_and_invalid_lists() {
        assert_eq!(ranges(None), vec![]);
        assert_eq!(
            ranges(Some("U+20, u+0-7F, U+4??")),
            vec![(0x20, 0x20), (0, 0x7F), (0x400, 0x4FF)]
        );
        assert_eq!(ranges(Some("U+7F-0")), vec![]);
        assert_eq!(ranges(Some("20-7F")), vec![]);
        assert!(covers(&[], 0x1F600));
        assert!(!covers(&[(0x400, 0x4FF)], 0x20));
    }

    #[test]
    fn subsets_with_the_chosen_descriptors_form_one_font_led_by_the_space() {
        let faces = vec![
            face("cyrillic", 400.0, vec![(0x400, 0x4FF)]),
            face("latin", 400.0, vec![(0, 0xFF)]),
            face("bold", 700.0, vec![(0, 0xFF)]),
            face("latin-ext", 400.0, vec![(0x100, 0x24F)]),
        ];
        let request = FontRequest::new("x", 16.0);
        let names: Vec<_> = composite(&faces, &request)
            .iter()
            .map(|f| f.family.as_str())
            .collect();
        // Later rules first, the space-holding subset in front; the bold rule is out.
        assert_eq!(names, ["latin", "latin-ext", "cyrillic"]);
    }
}
