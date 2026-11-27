//! KaminIDE patch: a quad with elliptical corners (`CornerEllipse`) draws
//! the pixels that a group of its square fill drew through the coverage
//! mask it replaces (`contour_dist` in kamin-html, corner shape 1).

use super::*;
use gpui::CornerEllipse;

/// Radii (horizontal, vertical) of the top left, top right, bottom right
/// and bottom left corners; they fit the box (no §5.5 reduction).
const RADII: [(f32, f32); 4] = [(40., 20.), (10., 35.), (60., 30.), (0., 0.)];
const BOX: [f32; 4] = [20., 30., 120., 70.];

/// Coverage at the pixel centre `(x, y)` of a `w` x `h` box: the mask's
/// formula, the first-order distance to the corner's ellipse or the
/// distance to the nearest side.
fn coverage(x: f32, y: f32, w: f32, h: f32) -> f32 {
    let corners = [(0.0, 0.0, 1.0, 1.0), (w, 0.0, -1.0, 1.0), (w, h, -1.0, -1.0), (0.0, h, 1.0, -1.0)];
    for (i, &(ox, oy, sx, sy)) in corners.iter().enumerate() {
        let (rx, ry) = RADII[i];
        if rx <= 0.0 || ry <= 0.0 {
            continue;
        }
        let (ex, ey) = (sx * (x - ox) / rx, sy * (y - oy) / ry);
        if ex < 0.0 || ey < 0.0 || ex >= 1.0 || ey >= 1.0 {
            continue;
        }
        let (dx, dy) = (1.0 - ex, 1.0 - ey);
        let dd = (dx * dx + dy * dy).sqrt();
        let grad = ((dx / rx) * (dx / rx) + (dy / ry) * (dy / ry)).sqrt() / dd.max(1e-6);
        return (0.5 - (dd - 1.0) / grad.max(1e-6)).clamp(0.0, 1.0);
    }
    let d = [-y, x - w, y - h, -x].into_iter().fold(f32::MIN, f32::max);
    (0.5 - d).clamp(0.0, 1.0)
}

fn fill(color: Hsla) -> Quad {
    let mut q = quad(BOX, color);
    q.corner_radii = Corners::default();
    q.order = 1;
    q
}

fn elliptical(color: Hsla) -> Scene {
    let mut s = Scene::default();
    let mut q = fill(color);
    let [tl, tr, br, bl] = RADII;
    q.corner_radii = Corners {
        top_left: ScaledPixels(tl.0),
        top_right: ScaledPixels(tr.0),
        bottom_right: ScaledPixels(br.0),
        bottom_left: ScaledPixels(bl.0),
    };
    q.ellipse = CornerEllipse { radii_y: [tl.1, tr.1, br.1, bl.1], on: 1, pad: [0; 3] };
    s.quads.push(q);
    s
}

fn masked(color: Hsla) -> Scene {
    let (w, h) = (BOX[2] as u32, BOX[3] as u32);
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let v = (coverage(x as f32 + 0.5, y as f32 + 0.5, w as f32, h as f32) * 255.0).round() as u8;
            bytes.extend_from_slice(&[v, v, v, v]);
        }
    }
    let area = b(BOX[0], BOX[1], BOX[2], BOX[3]);
    let mut inner = Scene::default();
    inner.quads.push(fill(color));
    let mut s = Scene::default();
    s.groups.push(PaintGroup {
        scene: std::rc::Rc::new(inner),
        bounds: area,
        mask: gpui::bgra_bytes_to_image(w, h, bytes),
        mask_bounds: area,
        mask_once: 3,
        ..Default::default()
    });
    s.surfaces.push(PaintSurface {
        order: 1,
        bounds: area,
        content_mask: mask(),
        corner_radii: Corners::default(),
        blur_radius: 0.,
        group: 1,
        opacity: 1.,
        color_matrix: None,
        fragment: 0,
    });
    s
}

fn drawn(device: &DirectXDevices, scene: Scene) -> Vec<u8> {
    let mut r = offscreen(device, W, H);
    let mut s = scene;
    s.note_frame_damage(&Scene::default(), 1, (W, H));
    r.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
    target_bytes(&mut r)
}

/// A circular quad of the existing path: the reference for how far two
/// adapters' blend units agree on antialiased edges.
fn circular(color: Hsla) -> Scene {
    let mut s = Scene::default();
    let mut q = fill(color);
    q.corner_radii = Corners { top_left: ScaledPixels(30.), top_right: ScaledPixels(10.), bottom_right: ScaledPixels(25.), bottom_left: ScaledPixels(0.) };
    s.quads.push(q);
    s
}

/// Pixels that differ and the largest difference, in codes.
fn compare(a: &[u8], b: &[u8]) -> (usize, u8) {
    (differing(a, b), a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0))
}

/// Opaque and translucent fills with elliptical corners draw the bytes of
/// their masked groups on the default adapter, and WARP draws the same
/// frame: the quad rounds every value to the target's 8-bit steps itself,
/// where the masked group leaves the rounding of its shader output to the
/// adapter (a GPU rounds before blending, WARP does not). The masked group
/// and a circular quad on the two adapters are counted for comparison;
/// `KAMIN_ELLIPSE_STATS` names a file that takes the counts.
#[test]
fn an_elliptical_quad_draws_the_pixels_of_its_mask() {
    let gpu = DirectXDevices::new().unwrap();
    let warp = DirectXDevices::with_instrumentation(crate::GpuAdapterPreference::Software, gpui::Instrumentation::default()).unwrap();
    let mut stats = Vec::new();
    for color in [hsla(0.6, 0.7, 0.5, 1.), hsla(0.1, 0.8, 0.45, 0.6)] {
        let mut frames = Vec::new();
        for (name, device) in [("gpu", &gpu), ("warp", &warp)] {
            let got = drawn(device, elliptical(color));
            let want = drawn(device, masked(color));
            let circle = drawn(device, circular(color));
            stats.push((format!("{name} alpha {} quad vs mask", color.a), compare(&got, &want)));
            frames.push((got, want, circle));
        }
        let ([g, w], [gm, wm], [gc, wc]) = (
            [&frames[0].0, &frames[1].0],
            [&frames[0].1, &frames[1].1],
            [&frames[0].2, &frames[1].2],
        );
        stats.push((format!("alpha {} quad gpu vs warp", color.a), compare(g, w)));
        stats.push((format!("alpha {} mask gpu vs warp", color.a), compare(gm, wm)));
        stats.push((format!("alpha {} circle gpu vs warp", color.a), compare(gc, wc)));
    }
    let lines: Vec<String> = stats.iter().map(|(name, (n, worst))| format!("{name}: {n} pixels, at most {worst} codes")).collect();
    if let Ok(path) = std::env::var("KAMIN_ELLIPSE_STATS") {
        std::fs::write(path, lines.join("\n")).unwrap();
    }
    let exact = |name: &str| stats.iter().filter(|(n, _)| n.contains(name)).all(|(_, (count, _))| *count == 0);
    assert!(exact("gpu alpha") && exact("quad gpu vs warp"), "{}", lines.join("; "));
}
