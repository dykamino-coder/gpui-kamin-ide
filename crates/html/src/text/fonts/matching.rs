//! CSS Fonts 4 §5.2: select one face before using its metrics and features.

use crate::style::computed::Computed;

#[derive(Clone)]
pub(crate) struct Face {
    pub width: (f32, f32),
    pub weight: (f32, f32),
    pub slope: u8,
    pub angle: (f32, f32),
    pub family: String,
    pub adjust: f32,
    pub features: Vec<(String, u32)>,
}

pub(super) fn range(value: Option<&str>, default: f32) -> (f32, f32) {
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

pub(crate) fn angle(value: &str) -> Option<f32> {
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

pub(super) fn slope(value: Option<&str>) -> (u8, (f32, f32)) {
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

fn style(face: &Face, request: &Computed) -> (u8, f32) {
    let italic = request.italic == Some(true) && request.oblique != Some(true);
    if italic {
        if face.slope == 1 {
            return (0, 0.0);
        }
        if request.font_oblique_only == Some(true) {
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
    if request.oblique != Some(true) {
        return match face.slope {
            0 => (0, 0.0),
            2 if face.angle.0 <= 0.0 && face.angle.1 >= 0.0 => (0, 0.0),
            1 => (2, 0.0),
            _ if face.angle.1 >= 0.0 => (1, face.angle.0.max(0.0)),
            _ => (3, -face.angle.1),
        };
    }
    let want = request.font_oblique_angle.unwrap_or(14.0);
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

pub(crate) fn select(family: &str, request: &Computed) -> Option<Face> {
    super::FACES.with(|faces| {
        let faces = faces.borrow();
        let faces = faces.get(&family.to_ascii_lowercase())?;
        let width = request.font_stretch.unwrap_or(100.0);
        let rank = |face: &Face| {
            [
                directed(width, face.width, width > 100.0),
                style(face, request),
                weight(request.font_weight.unwrap_or(400) as f32, face.weight),
            ]
        };
        // Later declarations win equal capabilities (composite subsets remain
        // ordered here; character coverage is handled separately by fallback).
        faces
            .iter()
            .rev()
            .min_by(|a, b| {
                rank(a)
                    .into_iter()
                    .zip(rank(b))
                    .map(|(a, b)| a.0.cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)))
                    .find(|order| !order.is_eq())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    })
}

pub(crate) fn alias_for(family: &str, request: &Computed) -> Option<String> {
    select(family, request)
        .map(|f| f.family)
        .or_else(|| super::alias(family))
}

/// Descriptor capabilities override binary style metadata: an oblique/bold
/// face supplies that style even if its OS/2 flags say normal.
pub(crate) fn features(family: &str, request: &Computed) -> Option<Vec<(String, u32)>> {
    let face = select(family, request)?;
    let mut out = face.features;
    let weight = request.font_weight.unwrap_or(400) as f32;
    // A regular request cannot need artificial bold. Keeping its flags equal
    // to a bold fallback request also preserves their shared shaping context.
    if weight != 400.0 && (face.weight.0..=face.weight.1).contains(&weight) {
        out.push(("nsyw".into(), 1));
    }
    if face.slope != 0
        || (request.font_oblique_only == Some(true)
            && request.italic == Some(true)
            && request.oblique != Some(true))
    {
        out.push(("nsys".into(), 1));
    }
    Some(out)
}

#[cfg(test)]
mod tests;
