//! Continuous circle/ellipse motion paths, including their true tangents.

use crate::style::computed::Computed;
use crate::style::values::value::Len;
use super::{origin_shift, rotation};
use std::f64::consts::FRAC_PI_2;

pub(super) fn css(
    c: &Computed,
    raw: &str,
    width: f32,
    height: f32,
    shift: (f32, f32),
) -> Option<String> {
    let (cx, cy, rx, ry) = crate::paint::background::shape_params(raw, width, height, 1.0)?;
    if rx <= 0.0 || ry <= 0.0 {
        return Some(origin_shift(
            c,
            (cx + shift.0, cy + shift.1),
            rotation(c, 0.0),
        ));
    }
    let (rx, ry) = (f64::from(rx), f64::from(ry));
    let quarter = if rx == ry {
        rx * FRAC_PI_2
    } else {
        integral(rx, ry, 0.0, FRAC_PI_2, 16)
    };
    let length = 4.0 * quarter;
    let requested = match c.offset_distance {
        Some(Len::Px(v)) => f64::from(v),
        Some(Len::Pct(p)) => f64::from(p) * length,
        Some(Len::Calc(i)) => crate::style::values::value::calc_get(i)
            .pct_px()
            .map_or(0.0, |(p, px)| f64::from(p) * length + f64::from(px)),
        _ => 0.0,
    };
    let distance = requested.rem_euclid(length);
    let quadrant = (distance / quarter).floor().min(3.0) as u8;
    let local = distance - f64::from(quadrant) * quarter;
    let angle = if local == 0.0 {
        0.0
    } else if rx == ry {
        local / rx
    } else if quadrant % 2 == 0 {
        angle_at(rx, ry, local, quarter)
    } else {
        FRAC_PI_2 - angle_at(rx, ry, quarter - local, quarter)
    };
    let (sin, cos) = angle.sin_cos();
    // Motion 1 sections 2.5 and 3: clockwise from the rightmost point;
    // auto rotation follows the curve derivative, not a flattening chord.
    // Blink computed_style.cc:1554-1580 requests a point and path tangent.
    let (ux, uy) = match quadrant {
        0 => (cos, sin),
        1 => (-sin, cos),
        2 => (-cos, -sin),
        _ => (sin, -cos),
    };
    let point = (
        (f64::from(cx) + rx * ux + f64::from(shift.0)) as f32,
        (f64::from(cy) + ry * uy + f64::from(shift.1)) as f32,
    );
    let tangent = (ry * ux).atan2(-rx * uy) as f32;
    Some(origin_shift(c, point, rotation(c, tangent)))
}

fn speed(rx: f64, ry: f64, angle: f64) -> f64 {
    let (sin, cos) = angle.sin_cos();
    (rx * sin).hypot(ry * cos)
}

fn integral(rx: f64, ry: f64, start: f64, end: f64, depth: u8) -> f64 {
    let mid = (start + end) * 0.5;
    let (a, b, c) = (speed(rx, ry, start), speed(rx, ry, mid), speed(rx, ry, end));
    let coarse = (end - start) * (a + 4.0 * b + c) / 6.0;
    let fine = (end - start)
        * (a + 4.0 * speed(rx, ry, (start + mid) * 0.5)
            + 2.0 * b
            + 4.0 * speed(rx, ry, (mid + end) * 0.5)
            + c)
        / 12.0;
    // Relative arc-length error stays well below the final f32 geometry.
    if depth == 0 || (fine - coarse).abs() <= 1e-10 * rx.max(ry) * (end - start) {
        fine + (fine - coarse) / 15.0
    } else {
        integral(rx, ry, start, mid, depth - 1) + integral(rx, ry, mid, end, depth - 1)
    }
}

fn angle_at(rx: f64, ry: f64, distance: f64, quarter: f64) -> f64 {
    if distance <= 0.0 {
        return 0.0;
    }
    if distance >= quarter {
        return FRAC_PI_2;
    }
    let (mut lo, mut hi) = (0.0, FRAC_PI_2);
    for _ in 0..40 {
        let mid = (lo + hi) * 0.5;
        if integral(rx, ry, 0.0, mid, 16) < distance {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) * 0.5
}
