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

/// Opaque and translucent fills with elliptical corners equal their masked
/// groups byte for byte, and a circle stays the circle of the plain quad.
#[test]
fn an_elliptical_quad_draws_the_pixels_of_its_mask() {
    let device = DirectXDevices::new().unwrap();
    for color in [hsla(0.6, 0.7, 0.5, 1.), hsla(0.1, 0.8, 0.45, 0.6)] {
        let got = drawn(&device, elliptical(color));
        let want = drawn(&device, masked(color));
        let worst = got.iter().zip(&want).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
        assert!(got == want, "{} pixels differ from the masked group (at most {worst} codes)", differing(&got, &want));
    }
}
