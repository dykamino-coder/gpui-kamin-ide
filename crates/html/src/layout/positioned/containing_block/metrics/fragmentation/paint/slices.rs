//! Срезы допуска по семьям: узлы семьи получают её размещения, а собственные
//! декорации коробок — геометрию фрагментов из тех же срезов.

use super::super::super::Snapshot;
use crate::hooks::SourceSlices;
use crate::layout::fragment::decoration::Decoration;
use gpui::PaintSegmentSlice;
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
                .collect::<Rc<[_]>>();
            Some((id, own))
        })
        .collect()
}
