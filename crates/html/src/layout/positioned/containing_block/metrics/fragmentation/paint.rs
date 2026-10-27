//! Допуск независимых OOF-продолжений: hit testing видит все размещения; запись источника
//! допускает только сегменты без legacy-копии предков, остальные воспроизводит копия.

use super::super::Snapshot;
use crate::hooks::SourceSlices;
use crate::layout::fragment::decoration::Decoration;
use crate::layout::fragment::paint_source::Nested;
use gpui::{
    Bounds, PaintRecordError, PaintSegment, PaintSegmentId, PaintSegmentSlice, Pixels, Point,
};
use std::{collections::HashMap, rc::Rc};

#[derive(Default)]
struct Own {
    routes: Vec<PaintSegment>,
    slices: Vec<PaintSegmentSlice>,
    decorations: Vec<PaintSegmentSlice>,
    bare: Vec<PaintSegmentId>,
}

/// Placements of continued absolute/fixed families, keyed (segment, ordinal).
#[derive(Default)]
pub(crate) struct Admission {
    ordinals: HashMap<PaintSegmentId, u32>,
    hits: Vec<PaintSegment>,
    orphan: Vec<PaintSegment>,
    hit_families: HashMap<u64, Vec<PaintSegmentSlice>>,
    orphan_families: HashMap<u64, Vec<PaintSegmentSlice>>,
    orphan_decorations: HashMap<u64, Vec<PaintSegmentSlice>>,
    own: HashMap<u64, Own>,
}

/// Routes and admission of one native source: hit testing, then paint.
pub(crate) struct Routing {
    pub(crate) hits: (Rc<[PaintSegment]>, SourceSlices),
    pub(crate) paint: (Rc<[PaintSegment]>, SourceSlices),
}

impl Admission {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// One fragment of `family` in `segment`: content admitted by `clip`, the
    /// box's own decoration by `decoration` (the fragment extent, css-break-3
    /// §5.4). `copied`: a legacy copy of the family's ancestors paints in that
    /// segment and replays the own record.
    pub(crate) fn insert(
        &mut self,
        family: u64,
        segment: PaintSegmentId,
        source: Point<Pixels>,
        destination: Point<Pixels>,
        clip: Bounds<Pixels>,
        decoration: Bounds<Pixels>,
        copied: bool,
    ) -> Result<(), PaintRecordError> {
        let ordinal = self.ordinals.entry(segment).or_default();
        let placement = PaintSegment {
            id: segment,
            ordinal: *ordinal,
            source,
            destination,
        };
        *ordinal = ordinal
            .checked_add(1)
            .ok_or(PaintRecordError::InvalidGeometry)?;
        let slice = PaintSegmentSlice {
            placement,
            source_clip: clip,
        };
        let decorated = PaintSegmentSlice {
            placement,
            source_clip: decoration,
        };
        self.hits.push(placement);
        self.hit_families.entry(family).or_default().push(slice);
        let own = self.own.entry(family).or_default();
        if copied {
            own.bare.push(segment);
            own.routes.push(placement);
            own.slices.push(slice);
            own.decorations.push(decorated);
        } else {
            // Source coordinates: the enclosing record re-admits and places it.
            let identity = PaintSegment {
                destination: source,
                ..placement
            };
            own.routes.push(identity);
            own.slices.push(PaintSegmentSlice {
                placement: identity,
                source_clip: clip,
            });
            own.decorations.push(PaintSegmentSlice {
                placement: identity,
                source_clip: decoration,
            });
            self.orphan_decorations
                .entry(family)
                .or_default()
                .push(decorated);
            self.orphan.push(placement);
            self.orphan_families.entry(family).or_default().push(slice);
        }
        Ok(())
    }

    pub(crate) fn finish(self, snapshot: &Snapshot) -> Routing {
        let families = Rc::new(snapshot.families.clone());
        let hits = slices(
            snapshot,
            &self.hit_families,
            &self.hit_families,
            families.clone(),
        );
        let mut paint = slices(
            snapshot,
            &self.orphan_families,
            &self.orphan_decorations,
            families,
        );
        for (root, own) in self.own {
            let extent = own
                .slices
                .iter()
                .map(|slice| slice.source_clip)
                .reduce(|a, b| a.union(&b))
                .unwrap_or_default();
            let shared: Rc<[PaintSegmentSlice]> = Rc::from(own.slices);
            let boxes = boxes(snapshot, root, &own.decorations);
            paint.nested.insert(
                root,
                Rc::new(Nested {
                    extent,
                    routes: Rc::from(own.routes),
                    slices: shared,
                    boxes,
                    bare: Rc::from(own.bare),
                }),
            );
        }
        Routing {
            hits: (Rc::from(self.hits), hits),
            paint: (Rc::from(self.orphan), paint),
        }
    }
}

fn slices(
    snapshot: &Snapshot,
    by_family: &HashMap<u64, Vec<PaintSegmentSlice>>,
    decorations: &HashMap<u64, Vec<PaintSegmentSlice>>,
    families: Rc<HashMap<u64, u64>>,
) -> SourceSlices {
    let by_family: HashMap<_, Rc<[_]>> = by_family
        .iter()
        .map(|(id, slices)| (*id, Rc::from(slices.as_slice())))
        .collect();
    let nodes: HashMap<_, _> = snapshot
        .families
        .iter()
        .map(|(&id, family)| {
            let slices = by_family.get(family).cloned();
            (id, slices.unwrap_or_else(|| Rc::from([])))
        })
        .collect();
    let mut boxes = HashMap::new();
    for (&family, slices) in decorations {
        boxes.extend(self::boxes(snapshot, family, slices));
    }
    // Normal-flow boxes stay on their legacy copies: no own decoration here.
    for id in nodes.keys() {
        boxes.entry(*id).or_insert_with(|| Rc::from([]));
    }
    SourceSlices {
        nodes,
        boxes,
        families,
        nested: HashMap::new(),
    }
}

fn boxes(
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

#[cfg(test)]
mod tests;
