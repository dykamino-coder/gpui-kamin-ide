//! Подбор лица `@font-face` по css-fonts-4 §5.2: среди правил одного семейства выбирается одно
//! лицо по ширине, затем наклону, затем весу (направленный поиск), прежде чем брать его метрики и
//! возможности. Здесь разбор диапазонов дескрипторов `font-weight`/`font-style`, выбор лица для
//! запроса и его возможности с подавлением синтеза (`nsyw`/`nsys`), когда дескриптор уже даёт
//! нужный стиль.

use super::{FontRequest, FontStyle};

#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    pub width: (f32, f32),
    pub weight: (f32, f32),
    pub slope: u8,
    pub angle: (f32, f32),
    pub family: String,
    pub adjust: f32,
    pub features: Vec<(String, u32)>,
    /// `unicode-range` of the rule; empty: every character.
    pub ranges: Vec<(u32, u32)>,
}

pub fn range(value: Option<&str>, default: f32) -> (f32, f32) {
    let Some(value) = value else {
        return (default, default);
    };
    let mut parts = value.split_whitespace().map(|v| match v {
        "normal" => Some(400.0),
        "bold" => Some(700.0),
        _ => v.parse::<f32>().ok(),
    });
    let Some(Some(lo)) = parts.next() else {
        return (default, default);
    };
    let hi = match parts.next() {
        Some(Some(value)) => value,
        None => lo,
        Some(None) => return (default, default),
    };
    if parts.next().is_some() || !(1.0..=1000.0).contains(&lo) || !(1.0..=1000.0).contains(&hi) {
        return (default, default);
    }
    (lo.min(hi), lo.max(hi))
}

pub fn angle(value: &str) -> Option<f32> {
    let (number, scale) = if let Some(v) = value.strip_suffix("deg") {
        (v, 1.0)
    } else if let Some(v) = value.strip_suffix("grad") {
        (v, 0.9)
    } else if let Some(v) = value.strip_suffix("rad") {
        (v, 180.0 / std::f32::consts::PI)
    } else {
        (value.strip_suffix("turn")?, 360.0)
    };
    let angle = number.parse::<f32>().ok()? * scale;
    (angle.is_finite() && (-90.0..=90.0).contains(&angle)).then_some(angle)
}

pub fn slope(value: Option<&str>) -> (u8, (f32, f32)) {
    let mut parts = value.unwrap_or("normal").split_whitespace();
    match parts.next() {
        Some("italic") => (1, (0.0, 0.0)),
        Some("oblique") => {
            let lo = parts.next().and_then(angle).unwrap_or(14.0);
            let hi = parts.next().and_then(angle).unwrap_or(lo);
            (2, (lo.min(hi), lo.max(hi)))
        }
        _ => (0, (0.0, 0.0)),
    }
}

fn directed(want: f32, range: (f32, f32), upward: bool) -> (u8, f32) {
    let v = want.clamp(range.0, range.1);
    (
        u8::from(if upward { v < want } else { v > want }),
        (v - want).abs(),
    )
}

fn weight(want: f32, range: (f32, f32)) -> (u8, f32) {
    let v = want.clamp(range.0, range.1);
    if (400.0..=500.0).contains(&want) {
        if (want..=500.0).contains(&v) {
            (0, v - want)
        } else if v < want {
            (1, want - v)
        } else {
            (2, v - 500.0)
        }
    } else {
        directed(want, range, want > 500.0)
    }
}

fn style(face: &Face, request: &FontRequest<'_>) -> (u8, f32) {
    let italic = request.style == FontStyle::Italic;
    if italic {
        if face.slope == 1 {
            return (0, 0.0);
        }
        if request.oblique_only {
            return if face.slope == 0 {
                (1, 0.0)
            } else {
                (2, (14.0 - 14.0f32.clamp(face.angle.0, face.angle.1)).abs())
            };
        }
        return if face.slope == 0 {
            (3, 0.0)
        } else if face.angle.1 > 0.0 {
            let rank = directed(11.0, face.angle, true);
            (1 + rank.0, rank.1)
        } else {
            (4, -face.angle.1)
        };
    }
    if !matches!(request.style, FontStyle::Oblique(_)) {
        return match face.slope {
            0 => (0, 0.0),
            2 if face.angle.0 <= 0.0 && face.angle.1 >= 0.0 => (0, 0.0),
            1 => (2, 0.0),
            _ if face.angle.1 >= 0.0 => (1, face.angle.0.max(0.0)),
            _ => (3, -face.angle.1),
        };
    }
    let want = match request.style {
        FontStyle::Oblique(angle) => angle,
        _ => 14.0,
    };
    if face.slope == 1 {
        return (4, 0.0);
    }
    let mut range = if face.slope == 0 {
        (0.0, 0.0)
    } else {
        face.angle
    };
    let want = if want < 0.0 {
        range = (-range.1, -range.0);
        -want
    } else {
        want
    };
    if range.1 > 0.0 || (want == 0.0 && range.1 >= 0.0) {
        let rank = directed(want, range, want >= 11.0);
        (rank.0, rank.1)
    } else if face.slope == 0 {
        (2, 0.0)
    } else {
        (5, -range.1)
    }
}

pub fn select(faces: &[Face], request: &FontRequest<'_>) -> Option<Face> {
    composite(faces, request).into_iter().next()
}

/// The faces of the best descriptor match, a composite face split by
/// `unicode-range` (css-fonts-4 §4.5 "composite fonts"): later declarations
/// first (§4.5: overlapping ranges are checked in reverse order), and the
/// first face whose range holds U+0020 leads, since it gives the first
/// available font (§5.1, Blink `CSSSegmentedFontFace::GetFontData`).
pub fn composite(faces: &[Face], request: &FontRequest<'_>) -> Vec<Face> {
    let width = request.width;
    let rank = |face: &Face| {
        [
            directed(width, face.width, width > 100.0),
            style(face, request),
            weight(request.weight, face.weight),
        ]
    };
    let order = |a: &Face, b: &Face| {
        rank(a)
            .into_iter()
            .zip(rank(b))
            .map(|(a, b)| a.0.cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)))
            .find(|order| !order.is_eq())
            .unwrap_or(std::cmp::Ordering::Equal)
    };
    let Some(best) = faces.iter().rev().min_by(|a, b| order(a, b)) else {
        return Vec::new();
    };
    let mut out: Vec<Face> = faces
        .iter()
        .rev()
        .filter(|f| order(f, best).is_eq())
        .cloned()
        .collect();
    if let Some(at) = out.iter().position(|f| f.covers(0x20)) {
        let lead = out.remove(at);
        out.insert(0, lead);
    }
    out
}

impl Face {
    /// The face's `unicode-range` holds `c` (no ranges: every character).
    pub fn covers(&self, c: u32) -> bool {
        self.ranges.is_empty() || self.ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&c))
    }
}

/// `unicode-range` descriptor value (css-fonts-4 §4.5): `U+20`, `U+0-7F`,
/// `U+4??`; an invalid descriptor is dropped and the default `U+0-10FFFF`
/// applies, given here as an empty list.
pub fn unicode_ranges(value: Option<&str>) -> Vec<(u32, u32)> {
    let Some(value) = value else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for part in value.split(',') {
        let part = part.trim();
        let Some(body) = part.strip_prefix("U+").or_else(|| part.strip_prefix("u+")) else {
            return Vec::new();
        };
        let hex = |s: &str| u32::from_str_radix(s.trim(), 16).ok();
        let (lo, hi) = match body.split_once('-') {
            Some((a, b)) => (hex(a), hex(b)),
            None if body.contains('?') => {
                (hex(&body.replace('?', "0")), hex(&body.replace('?', "F")))
            }
            None => (hex(body), hex(body)),
        };
        match (lo, hi) {
            (Some(lo), Some(hi)) if lo <= hi => out.push((lo, hi.min(0x10FFFF))),
            _ => return Vec::new(),
        }
    }
    out
}

/// Descriptor capabilities override binary style metadata: an oblique/bold
/// face supplies that style even if its OS/2 flags say normal.
pub fn features(face: Face, request: &FontRequest<'_>) -> Vec<(String, u32)> {
    let mut out = face.features;
    let weight = request.weight;
    // A regular request cannot need artificial bold. Keeping its flags equal
    // to a bold fallback request also preserves their shared shaping context.
    if weight != 400.0 && (face.weight.0..=face.weight.1).contains(&weight) {
        out.push(("nsyw".into(), 1));
    }
    if face.slope != 0
        || (request.oblique_only
            && request.style == FontStyle::Italic
            && !matches!(request.style, FontStyle::Oblique(_)))
    {
        out.push(("nsys".into(), 1));
    }
    out
}
