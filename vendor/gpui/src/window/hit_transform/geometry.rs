//! Прямое преобразование углов layout-коробки без paint- и hit-test-обрезки.

use super::*;

impl Window {
    /// Map all four corners through ancestor transforms, then take their bounds.
    /// A fragment may be presented under a scope different from its source.
    pub fn layout_bounds_in_viewport(
        &self,
        bounds: Bounds<Pixels>,
        fragment: Option<crate::PaintSegment>,
    ) -> Bounds<Pixels> {
        let scopes = self.render_contexts.get::<SegmentHitScopes>();
        let scope = fragment.and_then(|route| {
            scopes.as_ref()?.0.iter()
                .find(|(id, ordinal, _)| *id == route.id && *ordinal == route.ordinal)
                .map(|(_, _, scope)| &scope.0)
        }).unwrap_or(&self.hit_scope);
        let mut corners = [bounds.origin, bounds.top_right(), bounds.bottom_left(), bounds.bottom_right()];
        let mut next = scope.as_deref();
        while let Some(scope) = next {
            let forward = match scope.inverse {
                Mapping::Invertible(inverse) => inverse.inverse().unwrap_or_default(),
                Mapping::Singular(forward) => forward,
            };
            for corner in &mut corners {
                *corner = forward.apply(*corner);
            }
            next = scope.parent.as_deref();
        }
        let mut min = corners[0];
        let mut max = min;
        for p in &corners[1..] {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        Bounds::new(min, size(max.x - min.x, max.y - min.y))
    }
}
