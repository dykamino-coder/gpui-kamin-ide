//! Geometry shared by the viewer chrome and the embedded CSS viewport.

pub const TOOLBAR_HEIGHT: f32 = 76.;
pub const SIDEBAR_WIDTH: f32 = 240.;
pub const PRESETS: [u32; 3] = [375, 800, 1440];

pub fn content(window: (f32, f32), folder: bool) -> (f32, f32) {
    (
        (window.0 - sidebar(folder)).max(0.),
        (window.1 - TOOLBAR_HEIGHT).max(0.),
    )
}

pub fn sidebar(folder: bool) -> f32 {
    if folder { SIDEBAR_WIDTH } else { 0. }
}

pub fn preset_window(width: u32, window_height: f32, folder: bool, scale: f32) -> (f32, f32) {
    // Native client rectangles use integer device pixels. Round outward, leaving
    // at most one device pixel beside the exact CSS-width DocumentView.
    (
        ((width as f32 + sidebar(folder)) * scale).ceil() / scale,
        window_height,
    )
}

pub fn matches_preset(window_width: f32, width: u32, folder: bool, scale: f32) -> bool {
    (window_width - preset_window(width, 0., folder, scale).0).abs() < 0.01
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_reserve_chrome_without_changing_height() {
        for folder in [false, true] {
            for preset in PRESETS {
                let window = preset_window(preset, 676., folder, 1.);
                assert_eq!(content(window, folder), (preset as f32, 600.));
            }
        }
    }

    #[test]
    fn fractional_dpi_keeps_exact_css_width_and_detects_manual_resize() {
        for scale in [1., 1.25, 1.5, 1.75, 2.] {
            for folder in [false, true] {
                for width in PRESETS {
                    let outer = preset_window(width, 676., folder, scale);
                    assert!(matches_preset(outer.0, width, folder, scale));
                    let available = content(outer, folder).0;
                    assert!(available >= width as f32 - 0.001);
                    assert!(available < width as f32 + 1. / scale + 0.001);
                    assert_eq!(outer.1, 676.);
                    assert!(!matches_preset(outer.0 + 1. / scale, width, folder, scale));
                }
            }
        }
    }

    #[test]
    fn small_windows_never_produce_negative_viewports() {
        assert_eq!(content((100., 20.), true), (0., 0.));
        assert_eq!(content((800.5, 676.25), false), (800.5, 600.25));
    }
}
