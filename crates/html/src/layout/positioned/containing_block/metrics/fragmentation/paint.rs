//! Допуск исходных узлов задаётся своим семейством normal-flow или OOF-токеном.

use super::super::Snapshot;
use crate::hooks::SourceSlices;
use gpui::{PaintRecordError, PaintSegmentSlice};
use std::{collections::HashMap, rc::Rc};

pub(crate) struct Admission {
    families: HashMap<u64, Vec<PaintSegmentSlice>>,
}

impl Admission {
    pub(crate) fn new() -> Self {
        Self {
            families: HashMap::new(),
        }
    }

    pub(crate) fn insert(
        &mut self,
        family: u64,
        slice: PaintSegmentSlice,
    ) -> Result<(), PaintRecordError> {
        let slices = self.families.entry(family).or_default();
        if slices.iter().any(|old| {
            old.placement.id == slice.placement.id
                && old.placement.ordinal == slice.placement.ordinal
        }) {
            return Err(PaintRecordError::InvalidGeometry);
        }
        slices.push(slice);
        Ok(())
    }

    pub(crate) fn nodes(self, snapshot: &Snapshot) -> SourceSlices {
        let families: HashMap<_, Rc<[_]>> = self
            .families
            .into_iter()
            .map(|(id, slices)| (id, Rc::from(slices)))
            .collect();
        let nodes: HashMap<_, _> = snapshot
            .families
            .iter()
            .map(|(&id, family)| {
                (
                    id,
                    families
                        .get(family)
                        .cloned()
                        .unwrap_or_else(|| Rc::from([])),
                )
            })
            .collect();
        let boxes = snapshot
            .normal
            .iter()
            .filter_map(|(&id, &bounds)| {
                let slices = nodes.get(&id)?;
                Some((
                    id,
                    slices
                        .iter()
                        .filter_map(|&slice| {
                            crate::layout::fragment::decoration::Decoration::for_slice(
                                bounds,
                                slice,
                                snapshot.cloned_boxes.get(&id).copied(),
                            )
                        })
                        .collect::<Rc<[_]>>(),
                ))
            })
            .collect();
        SourceSlices {
            nodes,
            boxes,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Bounds, PaintSegment, PaintSegmentId, point, px, size};

    #[test]
    fn normal_slice_does_not_admit_the_oof_family_or_its_normal_children() {
        let mut snapshot = Snapshot::default();
        snapshot
            .families
            .extend([(10, 10), (11, 10), (20, 20), (21, 20), (30, 30)]);
        let slice = |id| PaintSegmentSlice {
            placement: PaintSegment {
                id: PaintSegmentId(id),
                ordinal: 0,
                source: point(px(0.), px(0.)),
                destination: point(px(100. * id as f32), px(0.)),
            },
            source_clip: Bounds::new(point(px(0.), px(0.)), size(px(100.), px(20.))),
        };
        let mut admission = Admission::new();
        admission.insert(10, slice(1)).unwrap();
        admission.insert(20, slice(2)).unwrap();
        let nodes = admission.nodes(&snapshot);
        assert_eq!(nodes.nodes[&10][0].placement.id, PaintSegmentId(1));
        assert!(Rc::ptr_eq(&nodes.nodes[&10], &nodes.nodes[&11]));
        assert_eq!(nodes.nodes[&20][0].placement.id, PaintSegmentId(2));
        assert!(Rc::ptr_eq(&nodes.nodes[&20], &nodes.nodes[&21]));
        assert!(nodes.nodes[&30].is_empty());
    }
}
