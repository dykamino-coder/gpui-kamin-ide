//! KaminIDE patch (R4): the retained target redrawn only within a frame's
//! damage (`retained.rs`) holds byte for byte what a whole redraw of that
//! frame draws, for every kind of edit the damage diff must see: a changed,
//! moved, inserted or removed primitive, a shadow, a group's content and
//! opacity, an unchanged frame and a chain of frames.

use super::*;
use gpui::{
    Background, Bounds, ContentMask, Corners, Hsla, PaintGroup, PaintSurface, Quad, ScaledPixels,
    SceneDamage, Shadow, hsla, point, size,
};
use windows::Win32::Graphics::Direct3D11::{D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE};

mod groups;
// KaminIDE patch: output-merger blend series equal their formulas.
mod tiles;

const W: u32 = 256;
const H: u32 = 192;

fn b(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: point(ScaledPixels(x), ScaledPixels(y)),
        size: size(ScaledPixels(w), ScaledPixels(h)),
    }
}

fn mask() -> ContentMask<ScaledPixels> {
    ContentMask {
        bounds: b(0., 0., W as f32, H as f32),
    }
}

fn quad(r: [f32; 4], color: Hsla) -> Quad {
    Quad {
        bounds: b(r[0], r[1], r[2], r[3]),
        content_mask: mask(),
        background: Background::from(color),
        corner_radii: Corners::all(ScaledPixels(4.)),
        ..Default::default()
    }
}

/// What a frame paints: a shadow, quads, and one group of two quads.
#[derive(Clone)]
struct Frame {
    shadow_blur: f32,
    quads: Vec<([f32; 4], Hsla)>,
    group_color: Hsla,
    group_opacity: f32,
}

impl Frame {
    fn base() -> Self {
        Frame {
            shadow_blur: 6.,
            quads: vec![
                ([10., 10., 60., 40.], hsla(0.0, 0.8, 0.5, 1.)),
                ([90., 20., 50., 50.], hsla(0.3, 0.8, 0.5, 1.)),
                ([200., 140., 40., 40.], hsla(0.6, 0.8, 0.5, 1.)),
                ([30., 120., 70., 30.], hsla(0.1, 0.8, 0.5, 0.6)),
            ],
            group_color: hsla(0.8, 0.7, 0.5, 1.),
            group_opacity: 0.7,
        }
    }

    fn scene(&self) -> Scene {
        let mut s = Scene::default();
        let mut order = 0;
        let mut next = || {
            order += 1;
            order
        };
        let bounds = b(20., 60., 50., 30.);
        s.shadows.push(Shadow {
            order: next(),
            blur_radius: ScaledPixels(self.shadow_blur),
            bounds,
            corner_radii: Corners::default(),
            content_mask: mask(),
            color: hsla(0., 0., 0., 0.5),
            element_bounds: bounds,
            element_corner_radii: Corners::default(),
            inset: 0,
            pad: 0,
        });
        for (r, c) in &self.quads {
            let mut q = quad(*r, *c);
            q.order = next();
            s.quads.push(q);
        }
        let mut inner = Scene::default();
        for (i, r) in [[150., 90., 40., 30.], [170., 100., 40., 30.]]
            .into_iter()
            .enumerate()
        {
            let mut q = quad(
                r,
                if i == 0 {
                    self.group_color
                } else {
                    hsla(0.5, 0.6, 0.4, 0.8)
                },
            );
            q.order = i as u32 + 1;
            inner.quads.push(q);
        }
        let group = b(150., 90., 60., 40.);
        s.groups.push(PaintGroup {
            scene: inner,
            bounds: group,
            ..Default::default()
        });
        s.surfaces.push(PaintSurface {
            order: next(),
            bounds: group,
            content_mask: mask(),
            corner_radii: Corners::default(),
            blur_radius: 0.,
            group: 1,
            opacity: self.group_opacity,
            color_matrix: None,
            fragment: 0,
        });
        s
    }
}

fn edits() -> Vec<(&'static str, Frame)> {
    let base = Frame::base();
    let with = |f: &dyn Fn(&mut Frame)| {
        let mut e = base.clone();
        f(&mut e);
        e
    };
    vec![
        ("colour", with(&|e| e.quads[2].1 = hsla(0.9, 0.8, 0.5, 1.))),
        ("move", with(&|e| e.quads[1].0[0] += 7.)),
        (
            "insert",
            with(&|e| {
                e.quads
                    .push(([220., 10., 20., 20.], hsla(0.2, 0.9, 0.5, 1.)))
            }),
        ),
        (
            "remove",
            with(&|e| {
                e.quads.remove(0);
            }),
        ),
        ("shadow", with(&|e| e.shadow_blur = 2.)),
        (
            "group content",
            with(&|e| e.group_color = hsla(0.4, 0.7, 0.5, 1.)),
        ),
        ("group opacity", with(&|e| e.group_opacity = 0.3)),
        ("unchanged", base.clone()),
    ]
}

/// The offscreen target's bytes (BGRA rows).
fn target_bytes(r: &mut DirectXRenderer) -> Vec<u8> {
    let staging = r.capture_staging().unwrap();
    let ctx = r.dev().device_context.clone();
    let target = r.res().render_target.clone().unwrap();
    let mut m = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe {
        ctx.CopyResource(&staging, &target);
        ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut m))
            .unwrap();
    }
    let row = (r.width * 4) as usize;
    let mut out = Vec::with_capacity(row * r.height as usize);
    for y in 0..r.height as usize {
        // SAFETY: the mapped staging texture is width x height BGRA8; each
        // row has RowPitch >= width * 4 readable bytes until Unmap.
        out.extend_from_slice(unsafe {
            std::slice::from_raw_parts((m.pData as *const u8).add(y * m.RowPitch as usize), row)
        });
    }
    unsafe { ctx.Unmap(&staging, 0) };
    out
}

/// `frames` drawn in sequence, each compared with the previous one; returns
/// the target after the last and the last frame's damage.
fn draw_chain(device: &DirectXDevices, frames: &[&Frame]) -> (Vec<u8>, SceneDamage) {
    let mut r = offscreen(device, W, H);
    let mut previous = Scene::default();
    let mut damage = SceneDamage::Unknown;
    for (i, f) in frames.iter().enumerate() {
        let mut s = f.scene();
        s.note_frame_damage(&previous, i as u64 + 1, (W, H));
        damage = s.frame_damage().0;
        r.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
        previous = s;
    }
    (target_bytes(&mut r), damage)
}

fn differing(a: &[u8], b: &[u8]) -> usize {
    a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count()
}

#[test]
fn a_frame_drawn_within_its_damage_equals_the_whole_frame() {
    let device = DirectXDevices::new().unwrap();
    let base = Frame::base();
    for (name, edit) in edits() {
        let (got, damage) = draw_chain(&device, &[&base, &edit]);
        let (want, whole) = draw_chain(&device, &[&edit]);
        assert_eq!(
            whole,
            SceneDamage::Unknown,
            "{name}: the reference is drawn whole"
        );
        match name {
            "unchanged" => assert_eq!(damage, SceneDamage::Unchanged, "{name}"),
            _ => assert!(
                matches!(damage, SceneDamage::Rect { width, height, .. } if width * height < W * H),
                "{name}: damage {damage:?} must be a part of the frame"
            ),
        }
        assert_eq!(got.len(), want.len());
        assert!(
            got == want,
            "{name}: {} pixels differ from the whole frame",
            differing(&got, &want)
        );
    }
}

#[test]
fn a_chain_of_edits_equals_its_last_frame_drawn_whole() {
    // Edits that stay in the lower right quarter, so every redraw is partial.
    let device = DirectXDevices::new().unwrap();
    let base = Frame::base();
    let mut chain = vec![base.clone()];
    for c in [0.9, 0.95, 0.35] {
        let mut e = chain[chain.len() - 1].clone();
        e.quads[2].1 = hsla(c, 0.8, 0.5, 1.);
        chain.push(e.clone());
        e.group_color = hsla(c, 0.7, 0.5, 1.);
        chain.push(e);
    }
    chain.push(chain[chain.len() - 1].clone());
    let frames: Vec<&Frame> = chain.iter().collect();
    for n in 2..=frames.len() {
        let (got, damage) = draw_chain(&device, &frames[..n]);
        assert!(
            matches!(
                damage,
                SceneDamage::Unchanged | SceneDamage::Rect { x: 128.., .. }
            ),
            "frame {n}: damage {damage:?}"
        );
        let (want, _) = draw_chain(&device, &[frames[n - 1]]);
        assert!(
            got == want,
            "frame {n}: {} pixels differ from the whole frame",
            differing(&got, &want)
        );
    }
}
mod sparse;
