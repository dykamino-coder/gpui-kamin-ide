//! Допуск независимых OOF-продолжений: hit testing видит все размещения; собственная запись
//! внешнего корня рисует его и вложенные продолжения, уже размещёнными. Сегмент без
//! legacy-копии предков допускает эту запись в записи источника тождественным размещением.

use super::super::Snapshot;
use crate::hooks::SourceSlices;
use crate::layout::fragment::decoration::Decoration;
use crate::layout::fragment::paint_source::Nested;
use gpui::{
    Bounds, PaintRecordError, PaintSegment, PaintSegmentId, PaintSegmentSlice, Pixels, Point,
};
use std::{collections::HashMap, rc::Rc};

/// Placements of one outermost root and every flow nested in it.
#[derive(Default)]
struct Own {
    routes: Vec<PaintSegment>,
    families: HashMap<u64, Vec<PaintSegmentSlice>>,
    decorations: HashMap<u64, Vec<PaintSegmentSlice>>,
    /// Segment, whether a legacy copy of the ancestors paints there, placed extent.
    segments: Vec<(PaintSegmentId, bool, Bounds<Pixels>)>,
    extent: Option<Bounds<Pixels>>,
}

/// Placements of continued absolute/fixed families, keyed (segment, ordinal).
#[derive(Default)]
pub(crate) struct Admission {
    ordinals: HashMap<PaintSegmentId, u32>,
    hits: Vec<PaintSegment>,
    hit_roots: Vec<(PaintSegmentId, u32, u64)>,
    hit_families: HashMap<u64, Vec<PaintSegmentSlice>>,
    own: Vec<(u64, Own)>,
}

/// Routes and admission of one native source: hit testing, then paint.
pub(crate) struct Routing {
    pub(crate) hits: (Rc<[PaintSegment]>, SourceSlices),
    pub(crate) paint: (Rc<[PaintSegment]>, SourceSlices),
    /// Hit placements (segment, ordinal) and the outermost root they belong to.
    pub(crate) roots: Vec<(PaintSegmentId, u32, u64)>,
}

/// One fragment of a continued flow.
pub(crate) struct Placement {
    pub(crate) family: u64,
    /// Outermost enclosing continued flow (the family itself at the top).
    pub(crate) root: u64,
    pub(crate) segment: PaintSegmentId,
    pub(crate) source: Point<Pixels>,
    pub(crate) destination: Point<Pixels>,
    /// Content admission in source coordinates.
    pub(crate) clip: Bounds<Pixels>,
    /// The box's own decoration: the whole fragment (css-break-3 §5.4).
    pub(crate) decoration: Bounds<Pixels>,
    /// A legacy copy of the root's ancestors paints in this segment.
    pub(crate) copied: bool,
}

impl Admission {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn ordinal(&mut self, segment: PaintSegmentId) -> Result<u32, PaintRecordError> {
        let ordinal = self.ordinals.entry(segment).or_default();
        let current = *ordinal;
        *ordinal = ordinal
            .checked_add(1)
            .ok_or(PaintRecordError::InvalidGeometry)?;
        Ok(current)
    }

    pub(crate) fn insert(&mut self, p: Placement) -> Result<(), PaintRecordError> {
        let placement = PaintSegment {
            id: p.segment,
            ordinal: self.ordinal(p.segment)?,
            source: p.source,
            destination: p.destination,
        };
        let slice = PaintSegmentSlice {
            placement,
            source_clip: p.clip,
        };
        let decorated = PaintSegmentSlice {
            placement,
            source_clip: p.decoration,
        };
        self.hits.push(placement);
        self.hit_roots.push((p.segment, placement.ordinal, p.root));
        self.hit_families.entry(p.family).or_default().push(slice);
        let index = match self.own.iter().position(|(root, _)| *root == p.root) {
            Some(index) => index,
            None => {
                self.own.push((p.root, Own::default()));
                self.own.len() - 1
            }
        };
        let own = &mut self.own[index].1;
        own.routes.push(placement);
        own.families.entry(p.family).or_default().push(slice);
        own.decorations.entry(p.family).or_default().push(decorated);
        let delta = p.destination - p.source;
        let placed = Bounds::new(p.clip.origin + delta, p.clip.size)
            .union(&Bounds::new(p.decoration.origin + delta, p.decoration.size));
        let source = p.clip.union(&p.decoration);
        own.extent = Some(own.extent.map_or(source, |e| e.union(&source)));
        match own.segments.iter_mut().find(|(s, _, _)| *s == p.segment) {
            Some(entry) => entry.2 = entry.2.union(&placed),
            None => own.segments.push((p.segment, p.copied, placed)),
        }
        Ok(())
    }

    pub(crate) fn finish(mut self, snapshot: &Snapshot) -> Routing {
        let families = Rc::new(snapshot.families.clone());
        let mut hits = slices(snapshot, &self.hit_families, families.clone());
        let mut paint = SourceSlices {
            families,
            ..Default::default()
        };
        let mut orphan_routes = Vec::new();
        for (root, _) in &self.own {
            let origin = snapshot
                .normal
                .get(root)
                .map(|b| b.origin)
                .unwrap_or_default()
                + snapshot.relative.get(root).copied().unwrap_or_default();
            paint.origins.insert(*root, origin);
            hits.origins.insert(*root, origin);
        }
        for (root, own) in std::mem::take(&mut self.own) {
            // Placed records re-enter the source record unmoved.
            let mut identity = Vec::new();
            for &(segment, copied, placed) in &own.segments {
                if copied {
                    continue;
                }
                let Ok(ordinal) = self.ordinal(segment) else {
                    continue;
                };
                let route = PaintSegment {
                    id: segment,
                    ordinal,
                    source: placed.origin,
                    destination: placed.origin,
                };
                orphan_routes.push(route);
                paint.placed = Some(paint.placed.map_or(placed, |p| p.union(&placed)));
                identity.push(PaintSegmentSlice {
                    placement: route,
                    source_clip: placed,
                });
            }
            paint.nodes.insert(root, Rc::from(identity));
            let mut boxes = HashMap::new();
            for (&family, slices) in &own.decorations {
                boxes.extend(self::boxes(snapshot, family, slices));
            }
            // Decoration layers are laid out at prepaint from the same placements.
            hits.boxes
                .extend(boxes.iter().map(|(id, own)| (*id, own.clone())));
            let bare = own
                .segments
                .iter()
                .filter(|(_, copied, _)| *copied)
                .map(|(segment, _, _)| *segment)
                .collect();
            paint.nested.insert(
                root,
                Rc::new(Nested {
                    extent: own.extent.unwrap_or_default(),
                    routes: Rc::from(own.routes),
                    families: own
                        .families
                        .into_iter()
                        .map(|(family, slices)| (family, Rc::from(slices)))
                        .collect(),
                    boxes,
                    bare,
                }),
            );
        }
        Routing {
            hits: (Rc::from(self.hits), hits),
            paint: (Rc::from(orphan_routes), paint),
            roots: self.hit_roots,
        }
    }
}

fn slices(
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
