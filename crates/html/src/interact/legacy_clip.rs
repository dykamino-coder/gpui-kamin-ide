//! Legacy rectangular clips use device pixel edges, including empty snapped regions.

const EMPTY: [f32; 4] = [-1.0e7, -1.0e7, 1.0, 1.0];

pub(super) fn resolve(edges: [Option<f32>; 4], width: f32, height: f32) -> [f32; 4] {
    let [top, right, bottom, left] = edges;
    let (top, left) = (top.unwrap_or(0.0), left.unwrap_or(0.0));
    let (right, bottom) = (right.unwrap_or(width), bottom.unwrap_or(height));
    if right <= left || bottom <= top {
        EMPTY
    } else {
        [left, top, right - left, bottom - top]
    }
}

pub(super) fn snap([x, y, width, height]: [f32; 4]) -> [f32; 4] {
    // Round endpoints after origin, shifts and device scale have been applied.
    // Rounding just the size loses the last column at fractional origins.
    // Adding a half in f32 loses it at large coordinates, including EMPTY.
    let edge = |value: f32| (f64::from(value) + 0.5).floor() as f32;
    let (left, top) = (edge(x), edge(y));
    let (right, bottom) = (edge(x + width), edge(y + height));
    if right <= left || bottom <= top {
        // The group shader interprets zero width as "no clip", so preserve
        // emptiness using the same offscreen region as an inverted CSS rect.
        EMPTY
    } else {
        [left, top, right - left, bottom - top]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_pixel_origin_retains_both_rounded_endpoints() {
        assert_eq!(snap([10.0, 89.0, 62.5, 62.5]), [10.0, 89.0, 63.0, 63.0]);
        assert_eq!(snap([10.5, 20.5, 62.5, 62.5]), [11.0, 21.0, 62.0, 62.0]);
        assert_eq!(snap([-10.5, -20.5, 62.5, 62.5]), [-10.0, -20.0, 62.0, 62.0]);
    }

    #[test]
    fn empty_subpixel_and_inverted_regions_cannot_disable_the_clip() {
        assert_eq!(resolve([None; 4], 80.0, 100.0), [0.0, 0.0, 80.0, 100.0]);
        assert_eq!(
            resolve([Some(20.0), None, Some(10.0), None], 80.0, 100.0),
            EMPTY
        );
        assert_eq!(snap([10.1, 20.1, 0.2, 10.0]), EMPTY);
        assert_eq!(snap(EMPTY), EMPTY);
    }
}
