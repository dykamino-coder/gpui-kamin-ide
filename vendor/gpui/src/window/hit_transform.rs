//! KaminIDE patch: hit testing of a transformed subtree through its inverse matrix.
//! The host prepaints the subtree once in its untransformed (local) coordinates;
//! hitboxes keep the scope chain and the frame maps the pointer back before testing.

use super::*;

/// Inverse mapping of one transformed subtree, recorded at prepaint.
#[derive(Debug)]
pub(crate) struct HitScope {
    parent: Option<Rc<HitScope>>,
    /// Parent-space logical pixels to local logical pixels; `None` is a singular matrix.
    inverse: Option<TransformationMatrix>,
    /// The parent's clip at scope entry, in parent space.
    clip: Bounds<Pixels>,
}

impl HitScope {
    /// The pointer in the scope's local coordinates, or `None` when an enclosing
    /// clip excludes it or a matrix in the chain is singular.
    pub(crate) fn local(scope: &HitScope, position: Point<Pixels>) -> Option<Point<Pixels>> {
        let mut chain: SmallVec<[&HitScope; 8]> = SmallVec::new();
        let mut next = Some(scope);
        while let Some(scope) = next {
            chain.push(scope);
            next = scope.parent.as_deref();
        }
        let mut point = position;
        for scope in chain.iter().rev() {
            if !scope.clip.contains(&point) {
                return None;
            }
            point = scope.inverse?.apply(point);
            if !point.x.0.is_finite() || !point.y.0.is_finite() {
                return None;
            }
        }
        Some(point)
    }
}

impl TransformationMatrix {
    /// The inverse affine map, or `None` for a singular or non-finite matrix.
    pub fn inverse(&self) -> Option<Self> {
        let [[a, b], [c, d]] = self.rotation_scale;
        let det = a * d - b * c;
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let rotation_scale = [[d / det, -b / det], [-c / det, a / det]];
        let [x, y] = self.translation;
        let translation = [
            -(rotation_scale[0][0] * x + rotation_scale[0][1] * y),
            -(rotation_scale[1][0] * x + rotation_scale[1][1] * y),
        ];
        let inverse = Self { rotation_scale, translation };
        (rotation_scale.iter().flatten().chain(&translation).all(|v| v.is_finite())).then_some(inverse)
    }
}

impl Window {
    /// Prepaint a subtree that is painted under `matrix` (logical pixels, local to
    /// parent). Hitboxes inserted inside are tested at the pointer mapped through
    /// the inverse matrix; content masks inside are local to the subtree, and the
    /// parent's clip still applies in parent space. A singular matrix makes the
    /// subtree's hitboxes unreachable.
    pub fn with_hitbox_transform<R>(
        &mut self,
        matrix: TransformationMatrix,
        prepaint: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.invalidator.debug_assert_prepaint();
        let scope = Rc::new(HitScope {
            parent: self.hit_scope.clone(),
            inverse: matrix.inverse(),
            clip: self.content_mask().bounds,
        });
        let saved = Box::new((
            self.hit_scope.replace(scope),
            std::mem::take(&mut self.content_mask_stack),
        ));
        let radius = f32::MAX / 64.0;
        self.content_mask_stack.push(ContentMask {
            bounds: Bounds::new(point(px(-radius), px(-radius)), size(px(radius * 2.0), px(radius * 2.0))),
        });
        self.with_restore(
            saved,
            |window, saved| {
                window.hit_scope = saved.0.take();
                window.content_mask_stack = std::mem::take(&mut saved.1);
            },
            prepaint,
        )
        .0
    }
}

/// The hit scope active at a prepaint point (e.g. inside a transformed copy that
/// replays a segment record); segment placements can be tested through it.
#[derive(Clone, Default)]
pub struct PaintHitScope(Option<Rc<HitScope>>);

struct SegmentHitScopes(Rc<[(crate::PaintSegmentId, u32, PaintHitScope)]>);

impl Window {
    /// The hit scope of the current prepaint position.
    pub fn paint_hit_scope(&self) -> PaintHitScope {
        PaintHitScope(self.hit_scope.clone())
    }

    /// Inside `with_prepaint_segments`, test the listed placements (segment,
    /// ordinal) through the given scopes instead of the current one: a placement
    /// replayed under a transformed copy is hit through that copy's matrix.
    pub fn with_prepaint_segment_hit_scopes<R>(
        &mut self,
        scopes: Rc<[(crate::PaintSegmentId, u32, PaintHitScope)]>,
        prepaint: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let _scopes = self.render_contexts.enter(Some(Rc::new(SegmentHitScopes(scopes))));
        prepaint(self)
    }

    /// Record the scope of the placement `route` about to be pushed.
    pub(crate) fn note_segment_hit_scope(&mut self, route: crate::PaintSegment) {
        let Some(scopes) = self.render_contexts.get::<SegmentHitScopes>() else { return };
        if let Some((_, _, scope)) = scopes.0.iter().find(|(id, ordinal, _)| *id == route.id && *ordinal == route.ordinal) {
            let entry = self.next_frame.hitboxes.len();
            self.next_frame.hitbox_entry_scopes.insert(entry, scope.0.clone());
        }
    }
}

impl HitScope {
    /// Whether `hitbox`, inserted under `scope`, covers the window point `position`.
    pub(crate) fn hitbox_contains(scope: Option<&Rc<HitScope>>, hitbox: &Hitbox, position: Point<Pixels>) -> bool {
        let position = match scope {
            Some(scope) => match HitScope::local(scope, position) {
                Some(local) => local,
                None => return false,
            },
            None => position,
        };
        hitbox.bounds.intersect(&hitbox.content_mask.bounds).contains(&position)
    }
}
