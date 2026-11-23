//! Physical inline text damage retained independently of CSSOM line fragments.
//! Missing pieces invalidate an owner for the entire recorded frame.

use gpui::{Bounds, ScaledPixels};
use kamin_heap::{HeapSize, Ops};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
struct Area(Option<Bounds<ScaledPixels>>);

impl HeapSize for Area {
    fn heap_size(&self, _: &mut Ops) -> usize {
        0
    }
}

#[derive(Debug, Default)]
pub(crate) struct PaintBounds {
    stamp: u64,
    owners: BTreeMap<u64, Area>,
}

impl HeapSize for PaintBounds {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        self.owners.heap_size(ops)
    }
}

impl PaintBounds {
    pub(crate) fn add(&mut self, stamp: u64, node: u64, area: Option<Bounds<ScaledPixels>>) {
        if self.stamp != stamp {
            self.owners.clear();
            self.stamp = stamp;
        }
        let area = area.filter(finite);
        self.owners
            .entry(node)
            .and_modify(|old| old.0 = old.0.zip(area).map(|(a, b)| a.union(&b)).filter(finite))
            .or_insert(Area(area));
    }

    pub(crate) fn area(
        &self,
        store: &crate::layout::reuse::Store,
        node: u64,
    ) -> Option<Bounds<ScaledPixels>> {
        store.fragments_hold(self.stamp).then_some(())?;
        self.owners.get(&node)?.0
    }
}

pub(crate) fn finite(area: &Bounds<ScaledPixels>) -> bool {
    [
        area.origin.x.0,
        area.origin.y.0,
        area.right().0,
        area.bottom().0,
    ]
    .into_iter()
    .all(f32::is_finite)
        && area.size.width.0 >= 0.
        && area.size.height.0 >= 0.
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, size};

    fn rect(y: f32) -> Bounds<ScaledPixels> {
        Bounds::new(
            point(ScaledPixels(1.), ScaledPixels(y)),
            size(ScaledPixels(5.), ScaledPixels(10.)),
        )
    }

    #[test]
    fn all_pieces_are_unioned_and_one_unknown_piece_poisons_the_owner() {
        let mut bounds = PaintBounds::default();
        bounds.add(1, 7, Some(rect(20.)));
        bounds.add(1, 7, Some(rect(40.)));
        assert_eq!(bounds.owners[&7].0, Some(rect(20.).union(&rect(40.))));
        bounds.add(1, 7, None);
        bounds.add(1, 7, Some(rect(50.)));
        assert!(bounds.owners[&7].0.is_none());
        bounds.add(2, 8, Some(rect(60.)));
        assert!(!bounds.owners.contains_key(&7));
        assert_eq!(bounds.stamp, 2);
    }

    #[test]
    fn non_finite_paint_cannot_prove_an_offscreen_region() {
        let mut bounds = PaintBounds::default();
        for y in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            bounds.add(1, 7, Some(rect(y)));
        }
        assert!(bounds.owners[&7].0.is_none());
        assert!(bounds.heap_size(&mut Ops::default()) > 0);
        bounds.add(2, 7, Some(rect(30.)));
        assert_eq!(bounds.owners[&7].0, Some(rect(30.)));
    }
}
