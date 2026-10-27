//! Clip принадлежит охватывающему scope, а не размещению его листьев.

use super::*;

/// Хранится независимо от маршрутов листьев, включая отложенный paint.
#[derive(Clone)]
pub(crate) struct Tag {
    pub(crate) routes: Routes,
    pub(crate) scopes: Routes,
    pub(crate) clips: Option<Rc<Clip>>,
    pub(crate) slices: Rc<[crate::PaintSegmentSlice]>,
}

pub(crate) struct Clip {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) routes: Routes,
    pub(crate) parent: Option<Rc<Clip>>,
}

impl Drop for Clip {
    fn drop(&mut self) {
        // Глубокая цепочка overflow освобождается без рекурсивного Drop.
        let mut parent = self.parent.take();
        while let Some(node) = parent {
            match Rc::try_unwrap(node) {
                Ok(mut node) => parent = node.parent.take(),
                Err(_) => break,
            }
        }
    }
}

impl Tag {
    pub(super) fn admission(&self, id: PaintSegmentId, scale: f32, extent: Bounds<ScaledPixels>) -> Bounds<ScaledPixels> {
        self.slices.iter().find(|slice| slice.placement.id == id).map_or(extent, |slice| {
            let route = slice.placement;
            extent.intersect(&Bounds::new(slice.source_clip.origin + route.destination - route.source, slice.source_clip.size).scale(scale))
        })
    }

    pub(super) fn clip_chain(
        chain: Option<&Rc<Clip>>, excluded: Option<&Rc<Clip>>,
        id: PaintSegmentId, scale: f32, mut extent: Bounds<ScaledPixels>,
    ) -> Bounds<ScaledPixels> {
        let mut scope = chain;
        while let Some(clip) = scope {
            if excluded.is_some_and(|excluded| Rc::ptr_eq(clip, excluded)) { break; }
            if let Some(route) = clip.routes.iter().find(|route| route.id == id) {
                let bounds = Bounds::new(clip.bounds.origin + route.destination - route.source, clip.bounds.size);
                extent = extent.intersect(&bounds.scale(scale));
            }
            scope = clip.parent.as_ref();
        }
        extent
    }
}
