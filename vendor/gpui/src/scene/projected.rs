//! KaminIDE patch: where a projected group (a plane of a 3D rendering
//! context) lands in the frame.
//!
//! A plane's `PaintGroup::projection` maps a frame pixel back into the
//! group's raster (`group_source_with` in the Windows shaders); the group's
//! image is nonzero only within its raster rectangle, so the frame pixels its
//! composite can colour are that rectangle's image under the inverse map.
//! Its mark is placed under the content mask of the 3D context, about the
//! whole window, so without this bound every plane damaged and composited a
//! window of pixels. cc bounds a render surface by its content rect mapped to
//! the target (`RenderSurfaceImpl::DrawableContentRect`), WebRender a picture
//! by its primitives' mapped bounds (`SpaceMapper::map`).

/// The bounding box (x0, y0, x1, y1) of `rect` (x0, y0, x1, y1 in the
/// raster) mapped into the frame by the inverse of `m` (frame to raster,
/// row-major 3×3); `None` for a singular map or when a corner lies behind
/// the eye, where a raster point's frame depth is not positive (the image is
/// then unbounded).
pub fn homography_image(m: &[f32; 9], rect: [f32; 4]) -> Option<[f32; 4]> {
    let p: [[f64; 3]; 3] = std::array::from_fn(|r| std::array::from_fn(|c| m[r * 3 + c] as f64));
    let cof =
        |r0: usize, r1: usize, c0: usize, c1: usize| p[r0][c0] * p[r1][c1] - p[r0][c1] * p[r1][c0];
    // The inverse is adj(P) / det(P).
    let adj = [
        [cof(1, 2, 1, 2), -cof(0, 2, 1, 2), cof(0, 1, 1, 2)],
        [-cof(1, 2, 0, 2), cof(0, 2, 0, 2), -cof(0, 1, 0, 2)],
        [cof(1, 2, 0, 1), -cof(0, 2, 0, 1), cof(0, 1, 0, 1)],
    ];
    let det = p[0][0] * adj[0][0] + p[0][1] * adj[1][0] + p[0][2] * adj[2][0];
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    let r = rect.map(|v| v as f64);
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for (x, y) in [(r[0], r[1]), (r[2], r[1]), (r[2], r[3]), (r[0], r[3])] {
        let h: [f64; 3] =
            std::array::from_fn(|k| (adj[k][0] * x + adj[k][1] * y + adj[k][2]) / det);
        // The shader keeps a frame pixel only where its raster depth (1 / h.z
        // here) is positive; positive at the corners, it is on the whole rect.
        if !h.iter().all(|v| v.is_finite()) || h[2] <= 1e-9 {
            return None;
        }
        for k in 0..2 {
            lo[k] = lo[k].min(h[k] / h[2]);
            hi[k] = hi[k].max(h[k] / h[2]);
        }
    }
    Some([lo[0] as f32, lo[1] as f32, hi[0] as f32, hi[1] as f32])
}

#[cfg(test)]
mod tests {
    use super::homography_image;

    #[test]
    fn a_raster_rectangle_maps_into_the_frame_by_the_inverse() {
        // Frame to raster: raster = frame / 2 - (50, 20).
        let m = [0.5, 0.0, -50.0, 0.0, 0.5, -20.0, 0.0, 0.0, 1.0];
        assert_eq!(
            homography_image(&m, [0.0, 0.0, 100.0, 50.0]),
            Some([100.0, 40.0, 300.0, 140.0])
        );
        // A perspective whose horizon crosses the rectangle: unbounded.
        let behind = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.02, 0.0, 1.0];
        assert_eq!(homography_image(&behind, [0.0, 0.0, 100.0, 50.0]), None);
        assert_eq!(homography_image(&[0.0; 9], [0.0, 0.0, 1.0, 1.0]), None);
    }
}
