//! CSS corner domains can cross the box midlines: compare the raster with
//! an independent contour mask, including the inner edge of unequal borders.

use super::*;

fn coverage(x: f32, y: f32, w: f32, h: f32, radii: [(f32, f32); 4]) -> f32 {
    for ((rx, ry), (vx, vy)) in radii.into_iter().zip([(x, y), (w - x, y), (w - x, h - y), (x, h - y)]) {
        if rx > 0. && ry > 0. && vx >= 0. && vy >= 0. && vx < rx && vy < ry {
            let (ex, ey) = (1. - vx / rx, 1. - vy / ry);
            let dd = (ex * ex + ey * ey).sqrt();
            let grad = ((ex / rx) * (ex / rx) + (ey / ry) * (ey / ry)).sqrt() / dd.max(1e-6);
            return (0.5 - (dd - 1.) / grad.max(1e-6)).clamp(0., 1.);
        }
    }
    (0.5 - (-x).max(-y).max(x - w).max(y - h)).clamp(0., 1.)
}

#[test]
fn corners_beyond_the_midlines_keep_their_whole_coverage() {
    let gpu = DirectXDevices::new().unwrap();
    let warp = DirectXDevices::with_instrumentation(crate::GpuAdapterPreference::Software, gpui::Instrumentation::default()).unwrap();
    for device in [&gpu, &warp] {
        for (w, h, radii) in [
            (180., 90., [(90., 90.), (90., 90.), (0., 0.), (0., 0.)]),
            (90., 160., [(80., 120.), (0., 0.), (10., 40.), (0., 0.)]),
            (140., 140., [(16., 16.), (80., 80.), (16., 16.), (80., 80.)]),
            (150., 100., [(120., 60.), (30., 20.), (90., 80.), (30., 40.)]),
        ] {
            // Rotation checks all four corners and both axes independently
            // of any page or the quadrant selected by the shader.
            for turn in 0..4 {
                let (w, h) = if turn % 2 == 0 { (w, h) } else { (h, w) };
                let radii: [(f32, f32); 4] = std::array::from_fn(|i| {
                    let (rx, ry) = radii[(i + 4 - turn) % 4];
                    if turn % 2 == 0 { (rx, ry) } else { (ry, rx) }
                });
                for edges in [[0.; 4], [3., 7., 11., 5.]] {
                    let [t, r, b, l] = edges;
                    let inner_radii = std::array::from_fn(|i| {
                        let (rx, ry) = radii[i];
                        ((rx - if i == 0 || i == 3 { l } else { r }).max(0.),
                         (ry - if i < 2 { t } else { b }).max(0.))
                    });
                    let mut q = quad([4., 4., w, h], hsla(0., 0., 0., 1.));
                    q.corner_radii = Corners { top_left: ScaledPixels(radii[0].0), top_right: ScaledPixels(radii[1].0), bottom_right: ScaledPixels(radii[2].0), bottom_left: ScaledPixels(radii[3].0) };
                    q.ellipse = gpui::CornerEllipse { radii_y: radii.map(|r| r.1), on: 1, ..Default::default() };
                    q.border_widths = gpui::Edges { top: ScaledPixels(t), right: ScaledPixels(r), bottom: ScaledPixels(b), left: ScaledPixels(l) };
                    q.border_color = hsla(0., 0., 1., 1.);
                    let mut scene = Scene::default();
                    scene.quads.push(q);
                    scene.note_frame_damage(&Scene::default(), 1, (W, H));
                    let mut renderer = offscreen(device, W, H);
                    renderer.draw(&scene, WindowBackgroundAppearance::Opaque).unwrap();
                    let got = target_bytes(&mut renderer);
                    for y in 0..h as usize {
                        for x in 0..w as usize {
                            let outer = coverage(x as f32 + 0.5, y as f32 + 0.5, w, h, radii);
                            let inner = coverage(x as f32 + 0.5 - l, y as f32 + 0.5 - t, w - l - r, h - t - b, inner_radii);
                            let m = (outer * 255.).round() as u32;
                            let color = if edges == [0.; 4] || outer == 0. { 0. } else { ((outer - inner) / outer).clamp(0., 1.) };
                            let p = (color * 255.).round() as u32;
                            let want = (255 - m + (p * m + 127) / 255) as u8;
                            let at = ((y + 4) * W as usize + x + 4) * 4;
                            assert_eq!(&got[at..at + 3], &[want; 3], "{w}x{h}, turn {turn}, edges {edges:?}, at {x},{y}");
                        }
                    }
                }
            }
        }
    }
}
