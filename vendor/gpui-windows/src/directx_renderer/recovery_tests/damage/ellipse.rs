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
    q.ellipse = CornerEllipse { radii_y: [tl.1, tr.1, br.1, bl.1], on: 1, ..Default::default() };
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

/// An opaque white box with a 9px red border on the right only, `double`
/// when `double`: the box spans x 20..140, so the lines are x 131..134 and
/// 137..140 with the gap 134..137.
fn bordered(double: bool) -> Scene {
    let mut s = elliptical(hsla(0., 0., 1., 1.));
    let q = &mut s.quads[0];
    q.border_widths = gpui::Edges { right: ScaledPixels(9.), ..Default::default() };
    q.border_color = hsla(0., 1., 0.5, 1.);
    q.ellipse.style = u32::from(double);
    s
}

/// A double border draws its two lines and leaves the gap between them to
/// the background; the antialiased pixels of the sides without width take
/// no border colour; GPU and WARP draw the same frame.
#[test]
fn an_elliptical_double_border_draws_two_lines_and_a_gap() {
    let gpu = DirectXDevices::new().unwrap();
    let warp = DirectXDevices::with_instrumentation(crate::GpuAdapterPreference::Software, gpui::Instrumentation::default()).unwrap();
    let plain = drawn(&gpu, elliptical(hsla(0., 0., 1., 1.)));
    for double in [false, true] {
        let frames = [&gpu, &warp].map(|d| drawn(d, bordered(double)));
        assert!(frames[0] == frames[1], "double {double}: {} pixels differ between the adapters", differing(&frames[0], &frames[1]));
        let at = |x: usize, y: usize| (y * W as usize + x) * 4;
        let px = |x: usize, y: usize| [frames[0][at(x, y)], frames[0][at(x, y) + 1], frames[0][at(x, y) + 2]];
        // Between the right corners (y 65..70): outer line, gap, inner line.
        let (red, white) = ([255, 0, 0], [255, 255, 255]);
        assert_eq!(px(138, 67), red, "double {double}: outer line");
        assert_eq!(px(135, 67), if double { white } else { red }, "double {double}: gap");
        assert_eq!(px(132, 67), red, "double {double}: inner line");
        assert_eq!(px(128, 67), white, "double {double}: background");
        // Left of the bottom right corner (x 80..140) the frame is the borderless box's,
        // antialiased corner pixels included.
        for y in 0..H as usize {
            for x in 0..80 {
                assert_eq!(frames[0][at(x, y)..at(x, y) + 4], plain[at(x, y)..at(x, y) + 4], "double {double}: border colour at {x},{y}");
            }
        }
    }
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
/// their masked groups on the default adapter and on WARP, and the two
/// adapters draw the same frame: the quad hands the blender the values the
/// group's composite hands it (8-bit buffer and mask, premultiplied). A
/// circular quad on the two adapters is counted for comparison;
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
    assert!(exact("quad vs mask") && exact("quad gpu vs warp"), "{}", lines.join("; "));
}
