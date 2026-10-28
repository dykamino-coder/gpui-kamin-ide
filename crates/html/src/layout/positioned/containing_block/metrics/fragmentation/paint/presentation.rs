//! Преобразование корня по фрагментам: `transform-origin` и доли сдвига решаются от
//! коробки каждого фрагмента (css-break-3 §5.5); у сегмента только с вложенными
//! продолжениями опорная коробка нулевой высоты в начале фрагментаинера.

use super::Placement;
use crate::style::computed::FlatTransform;
use gpui::{Bounds, PaintSegmentId, Pixels, Point, TransformationMatrix, point, px, size};
use std::collections::HashMap;

/// Fragment frames of one continued root with a flat transform.
#[derive(Default)]
pub(super) struct Frames {
    transform: Option<FlatTransform>,
    /// The root's own fragment (source) and its placement delta, per segment.
    own: HashMap<PaintSegmentId, (Bounds<Pixels>, Point<Pixels>)>,
    /// Source point of the fragmentainer start and the placement delta.
    starts: HashMap<PaintSegmentId, (Point<Pixels>, Point<Pixels>)>,
    inline: Option<Bounds<Pixels>>,
}

impl Frames {
    pub(super) fn note(&mut self, p: &Placement) {
        let delta = p.destination - p.source;
        if p.family == p.root {
            self.transform = p.transform;
            self.inline = Some(p.reference);
            self.own.entry(p.segment).or_insert((p.reference, delta));
        }
        self.starts.entry(p.segment).or_insert((p.start, delta));
    }

    /// The presentation of each segment (logical, destination coordinates):
    /// the root's matrix about that segment's fragment, applied to the placed
    /// record (`T(d)·M·T(-d)`).
    pub(super) fn matrices(&self, vertical: bool) -> HashMap<PaintSegmentId, TransformationMatrix> {
        let (Some(transform), Some(inline)) = (self.transform, self.inline) else {
            return HashMap::new();
        };
        self.starts
            .iter()
            .map(|(&segment, &(start, start_delta))| {
                let (reference, delta) = self.own.get(&segment).copied().unwrap_or_else(|| {
                    // Only nested continuations here: an empty fragment of the
                    // root at the fragmentainer start (Blink's empty fragment).
                    let origin = if vertical {
                        point(start.x, inline.origin.y)
                    } else {
                        point(inline.origin.x, start.y)
                    };
                    let extent = if vertical {
                        size(px(0.), inline.size.height)
                    } else {
                        size(inline.size.width, px(0.))
                    };
                    (Bounds::new(origin, extent), start_delta)
                });
                (segment, around(transform.matrix(reference), delta))
            })
            .collect()
    }
}

fn around(m: TransformationMatrix, d: Point<Pixels>) -> TransformationMatrix {
    let (dx, dy) = (f32::from(d.x), f32::from(d.y));
    let [[a, b], [c, e]] = m.rotation_scale;
    TransformationMatrix {
        rotation_scale: m.rotation_scale,
        translation: [
            m.translation[0] + dx - a * dx - b * dy,
            m.translation[1] + dy - c * dx - e * dy,
        ],
    }
}

/// Bounds of `b` under `m` (four corners).
pub(super) fn image(m: &TransformationMatrix, b: Bounds<Pixels>) -> Bounds<Pixels> {
    let corners = [
        b.origin,
        point(b.right(), b.top()),
        point(b.left(), b.bottom()),
        point(b.right(), b.bottom()),
    ]
    .map(|c| m.apply(c));
    let (mut lo, mut hi) = (corners[0], corners[0]);
    for c in corners {
        lo = point(lo.x.min(c.x), lo.y.min(c.y));
        hi = point(hi.x.max(c.x), hi.y.max(c.y));
    }
    Bounds::new(lo, size(hi.x - lo.x, hi.y - lo.y))
}
