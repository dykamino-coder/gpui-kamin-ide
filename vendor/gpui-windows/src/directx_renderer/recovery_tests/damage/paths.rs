//! KaminIDE patch: path batches redrawn within a rectangle (`path_pass`): a
//! batch that misses the rectangle leaves its intermediate untouched, and the
//! frame holds byte for byte what a whole redraw of it draws.

use super::*;
use gpui::{Path, Pixels, px};

/// A closed shape with a curved edge, `side` pixels wide, at (`x`, `y`).
fn shape(x: f32, y: f32, side: f32, order: u32, color: Hsla) -> Path<ScaledPixels> {
    let at = |dx: f32, dy: f32| point(px(x + dx), px(y + dy));
    let mut p = Path::<Pixels>::new(at(0., 0.));
    p.line_to(at(side, 0.));
    p.curve_to(at(side, side), at(side * 1.3, side * 0.5));
    p.line_to(at(0., side));
    p.line_to(at(0., 0.));
    p.color = Background::from(color);
    p.content_mask = ContentMask {
        bounds: Bounds {
            origin: point(px(0.), px(0.)),
            size: size(px(W as f32), px(H as f32)),
        },
    };
    let mut p = p.scale(1.);
    p.order = order;
    p
}

/// What a frame paints: quads under and between two path batches. Edits
/// change `color`, the quad at the top right, or move the second path.
#[derive(Clone, Copy)]
struct Spec {
    color: Hsla,
    second_x: f32,
}

impl Spec {
    fn scene(self) -> Scene {
        let mut s = Scene::default();
        for (r, c, order) in [
            ([0., 0., W as f32, H as f32], hsla(0., 0., 1., 1.), 1),
            ([60., 30., 40., 40.], hsla(0.3, 0.8, 0.5, 0.7), 3),
            ([200., 20., 30., 30.], self.color, 5),
        ] {
            let mut q = quad(r, c);
            q.order = order;
            s.quads.push(q);
        }
        s.paths
            .push(shape(10., 10., 50., 2, hsla(0.6, 0.8, 0.4, 0.8)));
        s.paths
            .push(shape(self.second_x, 100., 40., 4, hsla(0.0, 0.8, 0.5, 1.)));
        s
    }
}

/// Draw `frames` in sequence; the target after the last one and the
/// `[gpu]` line of the last one.
fn chain(device: &DirectXDevices, frames: &[Spec]) -> (Vec<u8>, String) {
    let mut r = offscreen(device, W, H);
    let mut previous = Scene::default();
    let mut line = String::new();
    for (i, f) in frames.iter().enumerate() {
        let mut s = f.scene();
        s.note_frame_damage(&previous, i as u64 + 1, (W, H));
        let _ = r.gpu_timer.instrumentation.batches.take_line();
        r.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
        line = r
            .gpu_timer
            .instrumentation
            .batches
            .take_line()
            .unwrap_or_default();
        previous = s;
    }
    (target_bytes(&mut r), line)
}

const BASE: Spec = Spec {
    color: Hsla {
        h: 0.1,
        s: 0.8,
        l: 0.5,
        a: 1.,
    },
    second_x: 120.,
};

#[test]
fn a_rectangle_away_from_the_paths_skips_their_batches_and_equals_the_whole_frame() {
    let device = DirectXDevices::new().unwrap();
    let edit = Spec {
        color: hsla(0.6, 0.9, 0.5, 1.),
        ..BASE
    };
    let (got, line) = chain(&device, &[BASE, edit]);
    let (want, whole) = chain(&device, &[edit]);
    assert!(line.contains("within=1"), "{line}");
    assert!(
        line.contains("b=2 mid_kpx=0"),
        "both path batches skipped: {line}"
    );
    assert!(
        !whole.contains("mid_kpx=0"),
        "the whole frame draws the paths: {whole}"
    );
    assert!(got == want, "{} pixels differ", differing(&got, &want));
}

#[test]
fn a_rectangle_across_a_path_redraws_its_part_and_equals_the_whole_frame() {
    let device = DirectXDevices::new().unwrap();
    // The edit moves the second path; its old and new places are the damage.
    let moved = Spec {
        second_x: 127.,
        ..BASE
    };
    let (got, line) = chain(&device, &[BASE, moved]);
    let (want, _) = chain(&device, &[moved]);
    assert!(line.contains("within=1"), "{line}");
    assert!(
        !line.contains("mid_kpx=0"),
        "the moved path is drawn: {line}"
    );
    assert!(got == want, "{} pixels differ", differing(&got, &want));
}
