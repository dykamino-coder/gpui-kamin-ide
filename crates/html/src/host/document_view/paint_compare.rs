//! Frame comparison for the document view tests: the painted quads with
//! their draw orders and drawn areas, and whether two frames draw the same
//! pixels.

use gpui::VisualTestContext;

/// The painted quads in paint order, without their draw orders; a pure
/// translation is folded into the bounds (a scrolled replay moves a quad by
/// its transformation, a fresh paint by its bounds: the same pixels).
pub(super) fn quads(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        window
            .painted_quads()
            .into_iter()
            .map(|mut q| {
                q.order = 0;
                let t = &mut q.transformation;
                if t.rotation_scale == [[1., 0.], [0., 1.]] {
                    q.bounds.origin.x.0 += t.translation[0];
                    q.bounds.origin.y.0 += t.translation[1];
                    t.translation = [0., 0.];
                }
                format!("{q:?}")
            })
            .collect()
    })
}

/// The painted quads with their draw orders and drawn areas (`quads`).
pub(super) fn painted(
    cx: &mut VisualTestContext,
) -> Vec<(String, u32, gpui::Bounds<gpui::ScaledPixels>)> {
    let keys = quads(cx);
    let drawn: Vec<_> = cx.update(|window, _| {
        window
            .painted_quads()
            .into_iter()
            .map(|q| {
                let t = q.transformation.translation;
                let mut b = q.bounds;
                b.origin.x.0 += t[0];
                b.origin.y.0 += t[1];
                (q.order, b.intersect(&q.content_mask.bounds))
            })
            .collect()
    });
    keys.into_iter()
        .zip(drawn)
        .map(|(key, (order, area))| (key, order, area))
        .collect()
}

/// Whether two frames draw the same pixels: the same quads, and every two
/// that overlap in the same order. Orders of disjoint quads are free (the
/// bounds tree orders a quad only above what it overlaps).
pub(super) fn same_paint(
    a: &[(String, u32, gpui::Bounds<gpui::ScaledPixels>)],
    b: &[(String, u32, gpui::Bounds<gpui::ScaledPixels>)],
) -> Result<(), String> {
    fn sorted(list: &[(String, u32, gpui::Bounds<gpui::ScaledPixels>)]) -> Vec<&String> {
        let mut keys: Vec<&String> = list.iter().map(|q| &q.0).collect();
        keys.sort();
        keys
    }
    let (ka, kb) = (sorted(a), sorted(b));
    if ka != kb {
        let at = (0..ka.len().max(kb.len())).find(|&i| ka.get(i) != kb.get(i));
        return Err(format!(
            "quads differ at sorted {at:?} of {}/{}: {:?} vs {:?}",
            ka.len(),
            kb.len(),
            at.and_then(|i| ka.get(i)),
            at.and_then(|i| kb.get(i))
        ));
    }
    let find = |list: &[(String, u32, gpui::Bounds<gpui::ScaledPixels>)], key: &String| {
        list.iter().find(|q| &q.0 == key).map(|q| q.1)
    };
    for (i, x) in b.iter().enumerate() {
        for y in &b[i + 1..] {
            if x.0 == y.0 || x.2.intersect(&y.2).is_empty() {
                continue;
            }
            let (ox, oy) = (find(a, &x.0), find(a, &y.0));
            if (x.1 < y.1) != (ox < oy) {
                return Err(format!("overlapping quads swap order: {} / {}", x.0, y.0));
            }
        }
    }
    Ok(())
}
