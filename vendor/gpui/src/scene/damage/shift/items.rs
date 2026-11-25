//! KaminIDE patch: the primitives a frame comparison reads, in paint order,
//! binned by damage cell.
//!
//! A layer whose content the frame records again unchanged (the same id and
//! version at the same place in the layer order of both frames) is the
//! engine telling the comparison that it paints the same: it is not expanded
//! into its primitives but stands as one marker item over the whole window
//! in both frames. Markers keep the paint order of what lies around them,
//! and two markers are equal only at the same place. A frame that records
//! its document layer anew without a change then costs the comparison of its
//! own primitives only, not of the document.

use super::super::{Area, DAMAGE_TILE, FULL, entry_area, paint_order};
use crate::scene::{Scene, SceneLayer};

/// One primitive in paint order: its scene, entry (`KIND_SHIFT` encoding)
/// and the area it may write, cut by `W`; or, with `marker` above 0, the
/// unchanged layer at place `marker - 1` of the layer order.
#[derive(Clone, Copy)]
pub(super) struct Item<'a> {
    pub(super) scene: &'a Scene,
    pub(super) entry: u32,
    pub(super) area: Area,
    pub(super) marker: u32,
}

impl Item<'_> {
    /// Whether `self` (new frame) and `other` (last frame) are known equal or
    /// known different by their markers; `None` when both are primitives.
    pub(super) fn same_marker(&self, other: &Item) -> Option<bool> {
        (self.marker != 0 || other.marker != 0).then_some(self.marker == other.marker)
    }
}

/// Places in the layer order where `scene` and `previous` draw the same
/// unchanged layer content (`drawn`), when both order the same number of layers.
pub(super) fn unchanged_layers(scene: &Scene, previous: &Scene) -> Vec<bool> {
    let (a, b) = (&scene.book.layer_order, &previous.book.layer_order);
    if a.len() != b.len() {
        return Vec::new();
    }
    a.iter()
        .zip(b)
        .map(|(&x, &y)| {
            let (sx, sy) = (&scene.book.layers[x as usize], &previous.book.layers[y as usize]);
            sx.drawn == sy.drawn
                && scene
                    .layer(x as usize)
                    .is_some_and(|c| c.scene.book.layers.is_empty())
        })
        .collect()
}

/// What `walk` meets in raster order.
enum Met<'a> {
    Primitive(&'a Scene, u32),
    /// The unchanged layer at this place of the layer order.
    Unchanged(usize),
}

/// Every primitive of `scene` in raster order, the content of its layers
/// (`layer(slot)`) at the layer's place, except the places `unchanged` marks;
/// `false` when a layer's content is unknown or holds layers itself.
fn walk<'a>(
    scene: &'a Scene,
    layer: &dyn Fn(usize) -> Option<&'a SceneLayer>,
    unchanged: &[bool],
    each: &mut dyn FnMut(Met<'a>),
) -> bool {
    let slots = &scene.book.layer_order;
    if slots.is_empty() {
        paint_order(scene, |e| each(Met::Primitive(scene, e)));
        return true;
    }
    // Layers are drawn as one batch at their order, after the primitives of
    // the same order (`PrimitiveKind::Layer` is the last kind).
    let mut own = Vec::new();
    paint_order(scene, |e| own.push(e));
    let order_of = |e: u32| super::super::entry_order(scene, e);
    let mut at = 0;
    for (place, &slot) in slots.iter().enumerate() {
        let key = scene.book.layers[slot as usize].order;
        while at < own.len() && order_of(own[at]) <= key {
            each(Met::Primitive(scene, own[at]));
            at += 1;
        }
        if unchanged.get(place).copied().unwrap_or(false) {
            each(Met::Unchanged(place));
            continue;
        }
        let Some(content) = layer(slot as usize) else {
            return false;
        };
        if !content.scene.book.layers.is_empty() {
            return false;
        }
        paint_order(&content.scene, |e| each(Met::Primitive(&content.scene, e)));
    }
    own[at..].iter().for_each(|&e| each(Met::Primitive(scene, e)));
    true
}

/// Every primitive of `scene` in raster order, layers expanded.
pub(super) fn each_primitive<'a>(
    scene: &'a Scene,
    layer: &dyn Fn(usize) -> Option<&'a SceneLayer>,
    each: &mut dyn FnMut(&'a Scene, u32),
) -> bool {
    walk(scene, layer, &[], &mut |met| {
        if let Met::Primitive(s, e) = met {
            each(s, e);
        }
    })
}

/// The items of `scene` (moved by `d`) that write inside `w`, the places
/// `unchanged` marks as markers; `Err` when a primitive reads the frame (a
/// backdrop) or a layer's content is unknown.
pub(super) fn items<'a>(
    scene: &'a Scene,
    layer: &dyn Fn(usize) -> Option<&'a SceneLayer>,
    unchanged: &[bool],
    d: (f32, f32),
    w: Area,
) -> Result<Vec<Item<'a>>, &'static str> {
    let mut out = Vec::new();
    let mut backdrop = !scene.hoisted_backdrops.is_empty();
    let complete = walk(scene, layer, unchanged, &mut |met| {
        let (s, entry) = match met {
            Met::Primitive(s, entry) => (s, entry),
            Met::Unchanged(place) => {
                let marker = place as u32 + 1;
                out.push(Item { scene, entry: 0, area: w, marker });
                return;
            }
        };
        let Some(a) = entry_area(s, entry) else {
            backdrop = true;
            return;
        };
        let a = if a.iter().any(|v| v.is_nan()) {
            FULL
        } else {
            a
        };
        let a = [
            (a[0] + d.0).max(w[0]),
            (a[1] + d.1).max(w[1]),
            (a[2] + d.0).min(w[2]),
            (a[3] + d.1).min(w[3]),
        ];
        if a[0] < a[2] && a[1] < a[3] {
            out.push(Item {
                scene: s,
                entry,
                area: a,
                marker: 0,
            });
        }
    });
    match (complete, backdrop) {
        (false, _) => Err("layer_unknown"),
        (_, true) => Err("backdrop"),
        _ => Ok(out),
    }
}
/// Cells of a `cols`×`rows` grid with the items that may write in each (CSR).
pub(super) fn bin(items: &[Item], cols: u32, rows: u32) -> (Vec<u32>, Vec<u32>) {
    let tile = DAMAGE_TILE as f32;
    let span = |a: &Area| {
        let c0 = (a[0] / tile).floor().clamp(0.0, (cols - 1) as f32) as u32;
        let r0 = (a[1] / tile).floor().clamp(0.0, (rows - 1) as f32) as u32;
        let c1 = ((a[2] / tile).ceil().clamp(1.0, cols as f32) as u32).max(c0 + 1);
        let r1 = ((a[3] / tile).ceil().clamp(1.0, rows as f32) as u32).max(r0 + 1);
        (c0, r0, c1, r1)
    };
    let mut starts = vec![0u32; (cols * rows) as usize + 1];
    for item in items {
        let (c0, r0, c1, r1) = span(&item.area);
        for r in r0..r1 {
            for c in c0..c1 {
                starts[(r * cols + c) as usize + 1] += 1;
            }
        }
    }
    for t in 0..(cols * rows) as usize {
        starts[t + 1] += starts[t];
    }
    let mut fill = starts.clone();
    let mut entries = vec![0u32; starts[(cols * rows) as usize] as usize];
    for (k, item) in items.iter().enumerate() {
        let (c0, r0, c1, r1) = span(&item.area);
        for r in r0..r1 {
            for c in c0..c1 {
                let t = (r * cols + c) as usize;
                entries[fill[t] as usize] = k as u32;
                fill[t] += 1;
            }
        }
    }
    (starts, entries)
}
