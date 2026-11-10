//! Прямое преобразование углов layout-коробки без paint- и hit-test-обрезки.

use super::*;

struct LayoutFragmentHitScopes {
    source: PaintHitScope,
    destinations: Rc<[(crate::PaintSegmentId, u32, PaintHitScope)]>,
}

#[derive(Debug)]
struct LayoutProjection {
    matrix: [[f32; 4]; 4],
    flatten: bool,
}

#[derive(Default)]
struct LayoutProjections(RefCell<std::collections::HashMap<usize, LayoutProjection>>);

impl Window {
    pub(in crate::window) fn with_layout_projections<R>(
        &mut self,
        query: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.with_render_context(Some(Rc::new(LayoutProjections::default())), query)
    }

    /// Query-only homogeneous transform; flat boxes flatten incoming child planes.
    /// The scope reuses the normal hit-scope payload, with no retained frame state.
    pub fn with_layout_projection<R>(
        &mut self,
        matrix: [[f32; 4]; 4],
        flatten: bool,
        query: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let Some(projections) = self.render_context::<LayoutProjections>() else {
            return self.with_layout_projections(|window| {
                window.with_layout_projection(matrix, flatten, query)
            });
        };
        let scope = Rc::new(HitScope {
            parent: self.hit_scope.clone(),
            inverse: Mapping::LayoutProjection,
            clip: self.content_mask().bounds,
        });
        // A retained source/destination scope keeps its address alive. A dead
        // scope's entry can be replaced if the allocator reuses that address.
        projections.0.borrow_mut().insert(
            Rc::as_ptr(&scope) as usize, LayoutProjection { matrix, flatten },
        );
        let saved = self.hit_scope.replace(scope);
        self.with_restore(saved, |window, saved| window.hit_scope = saved.take(), query).0
    }

    /// Query-only routing: retain source-local transforms below this entry point.
    pub fn with_layout_fragment_hit_scopes<R>(
        &mut self,
        destinations: Rc<[(crate::PaintSegmentId, u32, PaintHitScope)]>,
        query: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let context = Rc::new(LayoutFragmentHitScopes {
            source: self.paint_hit_scope(),
            destinations,
        });
        self.with_render_context(Some(context), query)
    }

    /// Map all four corners through ancestor transforms, then take their bounds.
    /// A fragment may be presented under a scope different from its source.
    pub fn layout_bounds_in_viewport(
        &self,
        bounds: Bounds<Pixels>,
        fragment: Option<crate::PaintSegment>,
    ) -> Bounds<Pixels> {
        self.layout_quad_in_viewport(
            [bounds.origin, bounds.top_right(), bounds.bottom_left(), bounds.bottom_right()],
            fragment,
        )
    }

    /// Map an already positioned quad without an intermediate axis-aligned union.
    pub fn layout_quad_in_viewport(
        &self,
        corners: [Point<Pixels>; 4],
        fragment: Option<crate::PaintSegment>,
    ) -> Bounds<Pixels> {
        let mut corners = corners.map(|p| [p.x.0, p.y.0, 0., 1.]);
        let query = self.render_contexts.get::<LayoutFragmentHitScopes>();
        let destination = fragment.and_then(|route| {
            query.as_ref()?.destinations.iter()
                .find(|(id, ordinal, _)| *id == route.id && *ordinal == route.ordinal)
                .map(|(_, _, scope)| scope)
        });
        let scopes = self.render_contexts.get::<SegmentHitScopes>();
        let scope = fragment.and_then(|route| {
            scopes.as_ref()?.0.iter()
                .find(|(id, ordinal, _)| *id == route.id && *ordinal == route.ordinal)
                .map(|(_, _, scope)| &scope.0)
        }).unwrap_or(&self.hit_scope);
        if let (Some(query), Some(destination)) = (&query, destination) {
            let mut next = self.hit_scope.as_ref();
            while let Some(scope) = next {
                if query.source.0.as_ref().is_some_and(|base| Rc::ptr_eq(base, scope)) {
                    break;
                }
                map_corners(self, scope, &mut corners);
                next = scope.parent.as_ref();
            }
            let mut next = destination.0.as_deref();
            while let Some(scope) = next {
                map_corners(self, scope, &mut corners);
                next = scope.parent.as_deref();
            }
        } else {
            let mut next = scope.as_deref();
            while let Some(scope) = next {
                map_corners(self, scope, &mut corners);
                next = scope.parent.as_deref();
            }
        }
        let corners = corners.map(|p| {
            let w = if p[3] == 0. { 1. } else { p[3] };
            point(px(p[0] / w), px(p[1] / w))
        });
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

fn map_corners(window: &Window, scope: &HitScope, corners: &mut [[f32; 4]; 4]) {
    let affine = match &scope.inverse {
        Mapping::Invertible(inverse) => inverse.inverse().unwrap_or_default(),
        Mapping::Singular(forward) => *forward,
        Mapping::LayoutProjection => {
            let Some(projections) = window.render_context::<LayoutProjections>() else { return };
            let projections = projections.0.borrow();
            let Some(projection) = projections.get(&(scope as *const HitScope as usize)) else { return };
            for corner in corners {
                if projection.flatten {
                    corner[2] = 0.;
                }
                let input = *corner;
                for (value, row) in corner.iter_mut().zip(projection.matrix) {
                    *value = row.iter().zip(input).map(|(a, b)| a * b).sum();
                }
            }
            return;
        }
    };
    for p in corners {
        let [x, y, _, w] = *p;
        p[0] = affine.rotation_scale[0][0] * x + affine.rotation_scale[0][1] * y + affine.translation[0] * w;
        p[1] = affine.rotation_scale[1][0] * x + affine.rotation_scale[1][1] * y + affine.translation[1] * w;
    }
}
