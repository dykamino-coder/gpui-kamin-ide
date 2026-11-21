//! KaminIDE patch: which group of the compared frame paints what a group of
//! this frame paints, so the renderer can keep that group's image.
//!
//! Groups are matched by content, not by index: inserting, removing or
//! reordering a node shifts the index of every later group, and an image
//! keyed by index would be lost for all of them. Old groups are bucketed by
//! a key of values every equal pair shares (bounds, blur and the primitive
//! counts of the group scene); a new group is checked exactly
//! (`Compare::same_group`) only against its bucket, the expected index (the
//! index shift of the last match) first. Every old group matches at most one
//! new group. The cost is O(groups) plus one exact comparison per match.

use super::{Compare, PaintGroup};
use collections::FxHashMap;

/// Values two groups that paint the same image always share.
fn key(g: &PaintGroup) -> [u32; 13] {
    let b = g.bounds;
    let s = &g.scene;
    [
        b.origin.x.0.to_bits(),
        b.origin.y.0.to_bits(),
        b.size.width.0.to_bits(),
        b.size.height.0.to_bits(),
        g.blur_radius.to_bits(),
        s.shadows.len() as u32,
        s.quads.len() as u32,
        s.paths.len() as u32,
        s.underlines.len() as u32,
        s.monochrome_sprites.len() as u32,
        s.subpixel_sprites.len() as u32,
        s.polychrome_sprites.len() as u32,
        s.surfaces.len() as u32,
    ]
}

/// For every group of `cmp.new`, the group of `cmp.old` that paints the same
/// image, plus one (0: none).
pub(super) fn match_groups(cmp: &mut Compare<'_>, out: &mut Vec<u32>) {
    out.clear();
    let (new, old) = (cmp.new, cmp.old);
    let mut buckets: FxHashMap<[u32; 13], Vec<u32>> = FxHashMap::default();
    for (i, g) in old.groups.iter().enumerate() {
        buckets.entry(key(g)).or_default().push(i as u32);
    }
    let mut taken = vec![false; old.groups.len()];
    let mut shift = 0i64;
    for (g, group) in new.groups.iter().enumerate() {
        let mut found = 0;
        if let Some(bucket) = buckets.get(&key(group)) {
            let expected = g as i64 + shift;
            let first = bucket.iter().position(|&o| i64::from(o) == expected);
            let order = first.into_iter().chain((0..bucket.len()).filter(|&k| Some(k) != first));
            for k in order {
                let o = bucket[k];
                if !taken[o as usize] && cmp.same_group(g as u32, o) {
                    taken[o as usize] = true;
                    shift = i64::from(o) - g as i64;
                    found = o + 1;
                    break;
                }
            }
        }
        out.push(found);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Background, Bounds, ContentMask, Corners, PaintGroup, PaintSurface, Quad, ScaledPixels, Scene, bounds, hsla, point, size};

    fn b(x: f32) -> Bounds<ScaledPixels> {
        bounds(point(ScaledPixels(x), ScaledPixels(0.)), size(ScaledPixels(10.), ScaledPixels(10.)))
    }

    /// One group per `(x, hue)`: a quad at `x` in a group at `x`.
    fn scene(groups: &[(f32, f32)]) -> Scene {
        let mut s = Scene::default();
        for (k, &(x, hue)) in groups.iter().enumerate() {
            let mut inner = Scene::default();
            inner.quads.push(Quad {
                order: 1,
                bounds: b(x),
                content_mask: ContentMask { bounds: b(x) },
                background: Background::from(hsla(hue, 0.5, 0.5, 1.)),
                ..Default::default()
            });
            s.groups.push(PaintGroup { scene: std::rc::Rc::new(inner), bounds: b(x), ..Default::default() });
            s.surfaces.push(PaintSurface { order: k as u32 + 1, bounds: b(x), content_mask: ContentMask { bounds: b(x) }, corner_radii: Corners::default(), blur_radius: 0., group: k as u32 + 1, opacity: 1., color_matrix: None, fragment: 0 });
        }
        s
    }

    fn bases(old: &[(f32, f32)], new: &[(f32, f32)]) -> Vec<Option<usize>> {
        let mut a = scene(old);
        a.note_frame_damage(&Scene::default(), 1, (256, 64));
        let mut s = scene(new);
        s.note_frame_damage(&a, 2, (256, 64));
        (0..new.len()).map(|g| s.group_base(g)).collect()
    }

    #[test]
    fn groups_match_by_content_across_removal_insertion_reordering_and_repaint() {
        let base: Vec<(f32, f32)> = (0..6).map(|i| (i as f32 * 20., i as f32 / 6.)).collect();
        let mut removed = base.clone();
        removed.remove(2);
        assert_eq!(bases(&base, &removed), [Some(0), Some(1), Some(3), Some(4), Some(5)]);
        assert_eq!(bases(&removed, &base), [Some(0), Some(1), None, Some(2), Some(3), Some(4)]);
        let mut swapped = base.clone();
        swapped.swap(1, 4);
        assert_eq!(bases(&base, &swapped), [Some(0), Some(4), Some(2), Some(3), Some(1), Some(5)]);
        let mut repainted = base.clone();
        repainted[3].1 = 0.9;
        assert_eq!(bases(&base, &repainted), [Some(0), Some(1), Some(2), None, Some(4), Some(5)]);
        // Two equal groups each take one old group, never the same one.
        let twins = [(0., 0.1), (0., 0.1)];
        assert_eq!(bases(&twins[..1], &twins), [Some(0), None]);
    }

    /// A group whose scene draws a retained layer (a replayed range) paints
    /// what the layer holds now, which its primitive lists do not show: it
    /// keeps no image of the last frame, whatever the lists say.
    #[test]
    fn a_group_drawing_a_retained_layer_matches_no_group() {
        use crate::scene::layer::{LayerHandle, SceneLayer};
        let layered = |hue: f32| {
            let mut content = Scene::default();
            content.insert_primitive(Quad {
                bounds: b(0.),
                content_mask: ContentMask { bounds: b(0.) },
                background: Background::from(hsla(hue, 0.5, 0.5, 1.)),
                ..Default::default()
            });
            let handle = LayerHandle::new(SceneLayer::finish(content, Vec::new(), Vec::new()));
            let mut s = scene(&[(0., 0.1)]);
            s.groups[0].scene_mut().insert_layer(&handle, b(0.));
            (s, handle)
        };
        let (mut a, _keep_a) = layered(0.2);
        a.note_frame_damage(&Scene::default(), 1, (256, 64));
        let (mut s, _keep_s) = layered(0.7);
        s.note_frame_damage(&a, 2, (256, 64));
        assert_eq!(s.group_base(0), None);
    }
}