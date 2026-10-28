//! Срезы допуска по семьям: узлы семьи получают её размещения, а собственные
//! декорации коробок — геометрию фрагментов из тех же срезов.

use super::super::super::Snapshot;
use crate::hooks::SourceSlices;
use crate::layout::fragment::decoration::Decoration;
use gpui::{Bounds, PaintSegmentId, PaintSegmentSlice, Pixels, Point, point, px};
use std::{collections::HashMap, rc::Rc};

pub(super) fn slices(
    snapshot: &Snapshot,
    by_family: &HashMap<u64, Vec<PaintSegmentSlice>>,
    families: Rc<HashMap<u64, u64>>,
) -> SourceSlices {
    let by_family: HashMap<_, Rc<[_]>> = by_family
        .iter()
        .map(|(id, slices)| (*id, Rc::from(slices.as_slice())))
        .collect();
    let nodes = snapshot
        .families
        .iter()
        .map(|(&id, family)| {
            let slices = by_family.get(family).cloned();
            (id, slices.unwrap_or_else(|| Rc::from([])))
        })
        .collect();
    SourceSlices {
        nodes,
        boxes: HashMap::new(),
        families,
        nested: HashMap::new(),
        placed: None,
        origins: HashMap::new(),
        presented: Default::default(),
    }
}

pub(super) fn boxes(
    snapshot: &Snapshot,
    family: u64,
    slices: &[PaintSegmentSlice],
    (exact, scale): (&HashMap<(PaintSegmentId, u32), Point<Pixels>>, f32),
) -> HashMap<u64, Rc<[Decoration]>> {
    snapshot
        .families
        .iter()
        .filter(|(_, f)| **f == family)
        .filter_map(|(&id, _)| {
            let bounds = *snapshot.normal.get(&id)?;
            let cloned = snapshot.cloned_boxes.get(&id).copied();
            let own = slices
                .iter()
                .filter_map(|&slice| Decoration::for_slice(bounds, slice, cloned))
                .map(|mut own| {
                    let at = (own.slice.placement.id, own.slice.placement.ordinal);
                    if let Some(&exact) = exact.get(&at) {
                        own.bounds = snapped(own.bounds, own.slice.placement, exact, scale);
                    }
                    own
                })
                .collect::<Rc<[_]>>();
            Some((id, own))
        })
        .collect()
}

/// Source bounds whose translation by the snapped placement lands on the device
/// edges of the exact destination, as a box painted there would snap each edge
/// (`Window::snap_bounds`): a translation alone keeps the source width and
/// leaves a device pixel between fragments at fractional column offsets.
fn snapped(
    bounds: Bounds<Pixels>,
    placement: gpui::PaintSegment,
    exact: Point<Pixels>,
    scale: f32,
) -> Bounds<Pixels> {
    if scale <= 0. {
        return bounds;
    }
    let exact = exact - placement.source;
    let moved = placement.destination - placement.source;
    let edge = |v: Pixels, exact: Pixels, moved: Pixels| {
        px(((f32::from(v + exact) * scale).round()) / scale) - moved
    };
    Bounds::from_corners(
        point(
            edge(bounds.left(), exact.x, moved.x),
            edge(bounds.top(), exact.y, moved.y),
        ),
        point(
            edge(bounds.right(), exact.x, moved.x),
            edge(bounds.bottom(), exact.y, moved.y),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{PaintSegment, size};

    #[test]
    fn decoration_lands_on_the_device_edges_of_its_exact_destination() {
        // A third of a 100px row: the column starts at 33.33 and ends at 66.67.
        // The translation snaps to 33, the box spans device pixels 33..67.
        let bounds = Bounds::new(point(px(0.), px(100.)), size(px(100. / 3.), px(100.)));
        let placement = PaintSegment {
            id: PaintSegmentId(1),
            ordinal: 0,
            source: point(px(0.), px(100.)),
            destination: point(px(33.), px(0.)),
        };
        let own = snapped(bounds, placement, point(px(100. / 3.), px(0.)), 1.);
        let moved = placement.destination - placement.source;
        assert_eq!(f32::from(own.left() + moved.x), 33.);
        assert_eq!(f32::from(own.right() + moved.x), 67.);
        assert_eq!(f32::from(own.top() + moved.y), 0.);
        assert_eq!(f32::from(own.bottom() + moved.y), 100.);
        // Without a device scale the bounds stay.
        assert_eq!(
            snapped(bounds, placement, point(px(100. / 3.), px(0.)), 0.),
            bounds
        );
    }
}
