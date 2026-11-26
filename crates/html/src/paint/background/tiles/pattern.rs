//! Which tiled backgrounds paint as one repeated image (`Window::with_sprite_pattern`).
//!
//! A tile laid out N×M times was N×M sprites: a `repeat-x` strip of a 1 px
//! wide image over a wide box drew about one instance per device column.
//! When the tiles abut and step by a whole number of device pixels, every
//! cell is the same snapped rectangle shifted by whole pixels, so one sprite
//! over the grid that samples the cell under each pixel draws the same
//! pixels. Other grids (gaps of `space`, fractional steps, vector and
//! gradient images, which are not sprites) keep one paint per tile.

use crate::paint::background::Source;
use crate::resources::css_images::Source as CpuSource;

/// The grid (columns, rows) of `xs`×`ys` tiles of `tile` size, when it
/// paints as one repeated image at device scale `sf`.
pub(super) fn pattern_grid(
    found: &Source,
    xs: &[f64],
    ys: &[f64],
    tile: (f32, f32),
    sf: f32,
) -> Option<(u32, u32)> {
    let sprite = matches!(
        found,
        Source::Image(CpuSource::Raster(_) | CpuSource::Filtered { .. })
    );
    grid(sprite, xs, ys, tile, sf)
}

fn grid(sprite: bool, xs: &[f64], ys: &[f64], tile: (f32, f32), sf: f32) -> Option<(u32, u32)> {
    if !sprite || xs.is_empty() || ys.is_empty() || xs.len() * ys.len() < 2 {
        return None;
    }
    let axis = |at: &[f64], len: f32| {
        if at.len() == 1 {
            return true;
        }
        let device = f64::from(len) * f64::from(sf);
        // Whole device pixels, and the drift over the strip stays far
        // below the snapping tolerance.
        let whole = (device - device.round()).abs() * at.len() as f64 <= 1e-3 && device >= 1.0;
        whole
            && at
                .windows(2)
                .all(|w| ((w[1] - w[0]) - f64::from(len)).abs() <= 1e-6 * f64::from(len).max(1.0))
    };
    if !axis(xs, tile.0) || !axis(ys, tile.1) {
        return None;
    }
    Some((u32::try_from(xs.len()).ok()?, u32::try_from(ys.len()).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abutting_whole_pixel_tiles_form_one_grid() {
        let xs: Vec<f64> = (0..800).map(f64::from).collect();
        assert_eq!(grid(true, &xs, &[0.0], (1.0, 1022.0), 1.0), Some((800, 1)));
        // 2.5 css px is a whole number of device pixels only at scale 2.
        let half: Vec<f64> = (0..4).map(|i| f64::from(i) * 2.5).collect();
        assert_eq!(grid(true, &half, &[0.0], (2.5, 4.0), 1.0), None);
        assert_eq!(grid(true, &half, &[0.0], (2.5, 4.0), 2.0), Some((4, 1)));
        // `space` leaves gaps between tiles.
        assert_eq!(grid(true, &[0.0, 12.0], &[0.0], (10.0, 10.0), 1.0), None);
        assert_eq!(grid(true, &[0.0], &[0.0], (10.0, 10.0), 1.0), None);
        assert_eq!(grid(false, &xs, &[0.0], (1.0, 1022.0), 1.0), None);
    }
}
