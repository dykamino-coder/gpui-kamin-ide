//! Допуск независимых OOF-продолжений: hit testing видит все размещения; собственная запись
//! внешнего корня рисует его и вложенные продолжения, уже размещёнными. Сегмент без
//! legacy-копии предков допускает эту запись в записи источника тождественным размещением.

use super::super::Snapshot;
use crate::hooks::SourceSlices;
use crate::layout::fragment::paint_source::Nested;
use gpui::{
    Bounds, PaintRecordError, PaintSegment, PaintSegmentId, PaintSegmentSlice, Pixels, Point,
    TransformationMatrix,
};
use std::{collections::HashMap, rc::Rc};
mod presentation;
mod slices;
use slices::{boxes, slices};

/// Placements of one outermost root and every flow nested in it.
#[derive(Default)]
struct Own {
    routes: Vec<PaintSegment>,
    families: HashMap<u64, Vec<PaintSegmentSlice>>,
    decorations: HashMap<u64, Vec<PaintSegmentSlice>>,
    /// Segment, whether a legacy copy of the ancestors paints there, placed extent.
    segments: Vec<(PaintSegmentId, bool, Bounds<Pixels>)>,
    extent: Option<Bounds<Pixels>>,
    frames: presentation::Frames,
}

/// Placements of continued absolute/fixed families, keyed (segment, ordinal).
#[derive(Default)]
pub(crate) struct Admission {
    ordinals: HashMap<PaintSegmentId, u32>,
    hits: Vec<PaintSegment>,
    hit_roots: Vec<(PaintSegmentId, u32, u64)>,
    hit_families: HashMap<u64, Vec<PaintSegmentSlice>>,
    own: Vec<(u64, Own)>,
    vertical: bool,
    /// Unsnapped destination of each placement and the device scale its own
    /// decorations snap at (`slices::boxes`).
    exact: HashMap<(PaintSegmentId, u32), Point<Pixels>>,
    scale: f32,
}

/// Routes and admission of one native source: hit testing, then paint.
pub(crate) struct Routing {
    pub(crate) hits: (Rc<[PaintSegment]>, SourceSlices),
    pub(crate) paint: (Rc<[PaintSegment]>, SourceSlices),
    /// Hit placements (segment, ordinal) and the outermost root they belong to.
    pub(crate) roots: Vec<(PaintSegmentId, u32, u64)>,
    /// Hit placements presented under their root's per-fragment transform.
    pub(crate) presented: Vec<(PaintSegmentId, u32, TransformationMatrix)>,
}

/// One fragment of a continued flow.
pub(crate) struct Placement {
    pub(crate) family: u64,
    /// Outermost enclosing continued flow (the family itself at the top).
    pub(crate) root: u64,
    pub(crate) segment: PaintSegmentId,
    pub(crate) source: Point<Pixels>,
    pub(crate) destination: Point<Pixels>,
    /// The destination before device snapping of the translation.
    pub(crate) exact: Point<Pixels>,
    /// Content admission in source coordinates.
    pub(crate) clip: Bounds<Pixels>,
    /// The box's own decoration: the whole fragment (css-break-3 §5.4).
    pub(crate) decoration: Bounds<Pixels>,
    /// A legacy copy of the root's ancestors paints in this segment.
    pub(crate) copied: bool,
    /// The family box's flat transform and its fragment here (source).
    pub(crate) transform: Option<crate::style::computed::FlatTransform>,
    pub(crate) reference: Bounds<Pixels>,
    /// Source point of this fragmentainer's start on the block axis.
    pub(crate) start: Point<Pixels>,
}

impl Admission {
    pub(crate) fn new(vertical: bool) -> Self {
        Self {
            vertical,
            ..Self::default()
        }
    }

    /// Own decorations land on the device edges of their exact destination.
    pub(crate) fn snapping(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
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
        self.exact.insert((p.segment, placement.ordinal), p.exact);
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
        own.frames.note(&p);
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
        let mut presented = Vec::new();
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
            let presentations = own.frames.matrices(self.vertical);
            if !presentations.is_empty() {
                paint.presented.insert(root);
                hits.presented.insert(root);
                presented.extend(own.routes.iter().filter_map(|route| {
                    let m = presentations.get(&route.id)?;
                    Some((route.id, route.ordinal, *m))
                }));
            }
            // Placed records re-enter the source record unmoved.
            let mut identity = Vec::new();
            for &(segment, copied, placed) in &own.segments {
                if copied {
                    continue;
                }
                let placed = presentations
                    .get(&segment)
                    .map_or(placed, |m| placed.union(&presentation::image(m, placed)));
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
                boxes.extend(self::boxes(
                    snapshot,
                    family,
                    slices,
                    (&self.exact, self.scale),
                ));
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
                    presentations,
                }),
            );
        }
        Routing {
            hits: (Rc::from(self.hits), hits),
            paint: (Rc::from(orphan_routes), paint),
            roots: self.hit_roots,
            presented,
        }
    }
}

#[cfg(test)]
mod tests;
