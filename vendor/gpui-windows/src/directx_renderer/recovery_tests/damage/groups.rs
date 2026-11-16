//! KaminIDE patch: group images kept between frames (`group_cache`) follow
//! the group's content, not its index — a removed, inserted or reordered
//! group keeps the images of the others — and the frame equals its whole
//! redraw.

use super::*;
use super::super::super::group_cache::GroupCounts;

/// One group per cell of a 4×3 grid: a quad of `colors[i]` at opacity 0.8.
fn groups(cells: &[(usize, Hsla)]) -> Scene {
    let mut s = Scene::default();
    for (k, &(cell, color)) in cells.iter().enumerate() {
        let (x, y) = ((cell % 4) as f32 * 60. + 8., (cell / 4) as f32 * 60. + 8.);
        let mut inner = Scene::default();
        let mut q = quad([x, y, 44., 44.], color);
        q.order = 1;
        inner.quads.push(q);
        let bounds = b(x, y, 44., 44.);
        s.groups.push(PaintGroup {
            scene: inner,
            bounds,
            ..Default::default()
        });
        s.surfaces.push(PaintSurface {
            order: k as u32 + 1,
            bounds,
            content_mask: mask(),
            corner_radii: Corners::default(),
            blur_radius: 0.,
            group: k as u32 + 1,
            opacity: 0.8,
            color_matrix: None,
            fragment: 0,
        });
    }
    s
}

fn color(i: usize) -> Hsla {
    hsla(i as f32 / 12., 0.7, 0.5, 1.)
}

/// Draw `frames` in sequence; the target after the last and its group counts.
fn chain(device: &DirectXDevices, frames: &[Vec<(usize, Hsla)>]) -> (Vec<u8>, GroupCounts) {
    let mut r = offscreen(device, W, H);
    let mut previous = Scene::default();
    for (i, f) in frames.iter().enumerate() {
        let mut s = groups(f);
        s.note_damage(&previous, i as u64 + 1, (W, H));
        r.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
        previous = s;
    }
    (target_bytes(&mut r), r.retained.groups.counts)
}

#[test]
fn group_images_follow_content_across_removal_insertion_and_reordering() {
    let device = DirectXDevices::new().unwrap();
    let base: Vec<(usize, Hsla)> = (0..12).map(|i| (i, color(i))).collect();
    let mut removed = base.clone();
    removed.remove(3);
    let mut reordered = base.clone();
    reordered.swap(2, 9);
    let mut recolored = base.clone();
    recolored[6].1 = hsla(0.95, 0.9, 0.4, 1.);
    let cases = [
        ("remove", vec![base.clone(), removed.clone()], 11, 0),
        ("insert", vec![base.clone(), removed, base.clone()], 11, 1),
        ("reorder", vec![base.clone(), reordered], 12, 0),
        ("recolor", vec![base.clone(), recolored], 11, 1),
    ];
    for (name, frames, kept, drawn) in cases {
        let (got, counts) = chain(&device, &frames);
        assert_eq!(
            counts,
            GroupCounts { kept, drawn },
            "{name}: groups kept and drawn"
        );
        let (want, whole) = chain(&device, &frames[frames.len() - 1..]);
        assert_eq!(whole.kept, 0, "{name}: the reference draws every group");
        assert!(
            got == want,
            "{name}: {} pixels differ from the whole frame",
            differing(&got, &want)
        );
    }
}

/// A kept image spoiled after it was drawn is drawn again within one round
/// of the scrub, and the comparison counts it once.
#[test]
fn a_spoiled_kept_image_is_redrawn_within_a_scrub_round_and_counted() {
    use windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext1;
    use windows::core::Interface;
    let device = DirectXDevices::new().unwrap();
    let base: Vec<(usize, Hsla)> = (0..12).map(|i| (i, color(i))).collect();
    let mut recolored = base.clone();
    recolored[6].1 = hsla(0.95, 0.9, 0.4, 1.);
    let mut r = offscreen(&device, W, H);
    let first = {
        let mut s = groups(&base);
        s.note_damage(&Scene::default(), 1, (W, H));
        r.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
        s
    };
    // Garbage over the atlas rectangle of group 0, as a failed draw would leave.
    let (place, region) = (r.blur.group_place[0], r.blur.group_regions[0]);
    let rect = windows::Win32::Foundation::RECT {
        left: place.x as i32,
        top: place.y as i32,
        right: (place.x + region.width) as i32,
        bottom: (place.y + region.height) as i32,
    };
    let dc: ID3D11DeviceContext1 = r.dev().device_context.cast().unwrap();
    let view = r.blur.groups[place.page].rtv[0].clone().unwrap();
    unsafe { dc.ClearView(&view, &[1.0, 0.0, 1.0, 1.0], Some(&[rect])) };
    let mut s = groups(&recolored);
    s.note_damage(&first, 2, (W, H));
    r.draw(&s, WindowBackgroundAppearance::Opaque).unwrap();
    assert_eq!(r.retained.groups.counts, GroupCounts { kept: 11, drawn: 1 });
    r.finish_scrub(true).unwrap();
    assert_eq!(r.retained.groups.scrub.mismatches, 1, "the spoiled image is counted once");
    let got = target_bytes(&mut r);
    let (want, _) = chain(&device, &[recolored]);
    assert!(got == want, "{} pixels differ from the whole frame", differing(&got, &want));
}