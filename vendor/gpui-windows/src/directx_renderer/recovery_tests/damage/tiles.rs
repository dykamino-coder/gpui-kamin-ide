//! KaminIDE patch: a composite run drawn in one pass (`group_tiles`) equals
//! the canonical group arithmetic: the Compositing formulas of its modes in
//! document order, rounded to 8 bits after every step.
//!
//! Oracle: every group drawn alone, normal, over a transparent frame gives
//! its premultiplied source image; the backdrop drawn alone gives the first
//! destination; `oracle` applies the formulas to those images. The pass must
//! equal it exactly on every adapter the tests reach (hardware and WARP): the
//! steps run in the shader, not in the adapter's blender. The composite
//! levels' distance to it is reported for comparison.

use super::*;
mod adapters;
mod oracle;
use adapters::devices;
use oracle::{expect, off};

/// One group: its quads (frame coordinates, colour), blend mode, opacity and blur.
#[derive(Clone)]
struct G {
    quads: Vec<([f32; 4], Hsla)>,
    blend: u32,
    opacity: f32,
    blur: f32,
}

fn g(x: f32, y: f32, c: Hsla, blend: u32) -> G {
    G {
        quads: vec![([x, y, 40., 30.], c), ([x + 12., y + 10., 30., 24.], hsla(0.55, 0.6, 0.5, 0.7))],
        blend,
        opacity: 1.,
        blur: 0.,
    }
}

fn plain(r: [f32; 4], c: Hsla, order: u32) -> Quad {
    let mut q = quad(r, c);
    q.corner_radii = Corners::default();
    q.order = order;
    q
}

/// Frame scene: backdrop quads, then the groups' composites; with `parent`,
/// the groups are children of one isolated normal group at that opacity.
fn scene(backdrop: &[([f32; 4], Hsla)], groups: &[G], parent: Option<f32>) -> Scene {
    let mut s = Scene::default();
    for (i, (r, c)) in backdrop.iter().enumerate() {
        s.quads.push(plain(*r, *c, i as u32 + 1));
    }
    let base = backdrop.len() as u32 + 1;
    let mut marks = Vec::new();
    for (k, grp) in groups.iter().enumerate() {
        let mut inner = Scene::default();
        for (i, (r, c)) in grp.quads.iter().enumerate() {
            inner.quads.push(plain(*r, *c, i as u32 + 1));
        }
        let bounds = b(0., 0., W as f32, H as f32);
        s.groups.push(PaintGroup {
            scene: inner,
            bounds,
            blur_radius: grp.blur,
            blend: grp.blend,
            ..Default::default()
        });
        marks.push(PaintSurface {
            order: base + k as u32,
            bounds,
            content_mask: mask(),
            corner_radii: Corners::default(),
            blur_radius: 0.,
            group: k as u32 + 1,
            opacity: grp.opacity,
            color_matrix: None,
            fragment: 0,
        });
    }
    match parent {
        None => s.surfaces = marks,
        Some(opacity) => {
            let mut inner = Scene::default();
            inner.surfaces = marks;
            let bounds = b(0., 0., W as f32, H as f32);
            s.groups.push(PaintGroup {
                scene: inner,
                bounds,
                ..Default::default()
            });
            s.surfaces.push(PaintSurface {
                order: base,
                bounds,
                content_mask: mask(),
                corner_radii: Corners::default(),
                blur_radius: 0.,
                group: groups.len() as u32 + 1,
                opacity,
                color_matrix: None,
                fragment: 0,
            });
        }
    }
    s
}

fn draw(device: &DirectXDevices, s: &Scene, bg: WindowBackgroundAppearance) -> Vec<u8> {
    draw_path(device, s, bg, true)
}

/// `tiles` false: the composite levels (one backdrop copy per level).
fn draw_path(device: &DirectXDevices, s: &Scene, bg: WindowBackgroundAppearance, tiles: bool) -> Vec<u8> {
    let mut r = offscreen(device, W, H);
    r.group_tiles = tiles;
    r.draw(s, bg).unwrap();
    target_bytes(&mut r)
}

/// The frame with its composite runs in tile passes (at least one drawn).
fn draw_tiles(device: &DirectXDevices, s: &Scene, bg: WindowBackgroundAppearance) -> Vec<u8> {
    let mut r = offscreen(device, W, H);
    r.draw(s, bg).unwrap();
    assert!(r.blur.tile_lists.passes > 0, "no tile pass drawn");
    target_bytes(&mut r)
}

/// The tile pass's frame against the levels' frame and both against the
/// formula: the pass equals the formula exactly and differs from the levels
/// by at most one code.
fn compare(name: &str, got: &[u8], levels: &[u8], want: &[u8]) {
    let (big, one) = off(got, levels);
    let (gb, go) = off(got, want);
    let (lb, lo) = off(levels, want);
    eprintln!(
        "{name}: tiles vs levels {one} channels off by one; vs formula: tiles {gb}/{go}, levels {lb}/{lo} (>1 / =1)"
    );
    assert_eq!(big, 0, "{name}: tiles off the levels by more than one");
    assert_eq!((gb, go), (0, 0), "{name}: tiles off the formula");
}

fn backdrop() -> Vec<([f32; 4], Hsla)> {
    vec![
        ([0., 0., 128., 192.], hsla(0.1, 0.8, 0.6, 1.)),
        ([96., 40., 120., 100.], hsla(0.6, 0.7, 0.4, 0.5)),
    ]
}

fn check(device: &DirectXDevices, name: &str, groups: &[G], bg: WindowBackgroundAppearance) {
    let frame = scene(&backdrop(), groups, None);
    let got = draw_tiles(device, &frame, bg);
    let copies = draw_path(device, &frame, bg, false);
    let base = draw(device, &scene(&backdrop(), &[], None), bg);
    let images: Vec<Vec<u8>> = groups
        .iter()
        .map(|grp| {
            let alone = G { blend: 0, opacity: 1., ..grp.clone() };
            draw(device, &scene(&[], &[alone], None), WindowBackgroundAppearance::Transparent)
        })
        .collect();
    let modes: Vec<u32> = groups.iter().map(|g| g.blend).collect();
    let opacity: Vec<f32> = groups.iter().map(|g| g.opacity).collect();
    let want = expect(base, &images, &modes, &opacity);
    compare(name, &got, &copies, &want);
}

fn chain(modes: &[u32]) -> Vec<G> {
    modes
        .iter()
        .enumerate()
        .map(|(i, &m)| g(10. + 28. * i as f32, 20. + 18. * i as f32, hsla(i as f32 / 7., 0.8, 0.5, 0.9), m))
        .collect()
}

#[test]
fn runs_equal_their_formulas_over_opaque_and_transparent_backdrops() {
    for (gpu, device) in devices() {
        for bg in [WindowBackgroundAppearance::Opaque, WindowBackgroundAppearance::Transparent] {
            check(&device, &format!("{gpu}: multiply chain {bg:?}"), &chain(&[1; 6]), bg);
            check(&device, &format!("{gpu}: mixed run {bg:?}"), &chain(&[2, 11, 16, 0, 1, 2]), bg);
        }
    }
}

#[test]
fn modes_of_one_run_interleave_in_document_order() {
    for (gpu, device) in devices() {
        let bg = WindowBackgroundAppearance::Opaque;
        check(&device, &format!("{gpu}: multiply/difference"), &chain(&[1, 10, 1, 0, 10, 1]), bg);
    }
}

/// Every separable and non-separable mode, with a translucent group in the run.
#[test]
fn every_blend_mode_equals_its_formula() {
    for (gpu, device) in devices() {
        let bg = WindowBackgroundAppearance::Opaque;
        for m in 1..=15 {
            let mut groups = chain(&[m, m, 3, m, 9, m]);
            groups[2].opacity = 0.6;
            check(&device, &format!("{gpu}: mode {m}"), &groups, bg);
        }
    }
}

#[test]
fn blurred_and_translucent_groups_equal_their_formulas() {
    for (gpu, device) in devices() {
        let mut groups = chain(&[1; 5]);
        groups[1].blur = 2.;
        groups[3].blur = 1.;
        groups[2].opacity = 0.35;
        check(&device, &format!("{gpu}: blurred multiply"), &groups, WindowBackgroundAppearance::Opaque);
    }
}

#[test]
fn children_of_an_isolated_group_blend_with_its_buffer_not_the_frame() {
    for (gpu, device) in devices() {
        let bg = WindowBackgroundAppearance::Opaque;
        let children = chain(&[1, 1, 2, 1]);
        let frame = scene(&backdrop(), &children, Some(1.));
        let got = draw_tiles(&device, &frame, bg);
        let copies = draw_path(&device, &frame, bg, false);
        // The isolated buffer starts transparent: the children blend with it alone.
        let images: Vec<Vec<u8>> = children
            .iter()
            .map(|c| {
                let alone = G { blend: 0, opacity: 1., ..c.clone() };
                draw(&device, &scene(&[], &[alone], None), WindowBackgroundAppearance::Transparent)
            })
            .collect();
        let modes: Vec<u32> = children.iter().map(|c| c.blend).collect();
        let opacity: Vec<f32> = children.iter().map(|c| c.opacity).collect();
        let isolated = expect(vec![0; (W * H * 4) as usize], &images, &modes, &opacity);
        let base = draw(&device, &scene(&backdrop(), &[], None), bg);
        let want = expect(base, &[isolated], &[0], &[1.]);
        compare(&format!("{gpu}: isolated"), &got, &copies, &want);
    }
}
