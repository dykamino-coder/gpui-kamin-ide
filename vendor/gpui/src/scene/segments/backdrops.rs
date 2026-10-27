//! Сегмент поднятой подложки сохраняется при переносе сквозь группы.

use super::*;

impl Scene {
    pub(crate) fn hoist_backdrop(&mut self, surface: PaintSurface) {
        self.hoisted_backdrops.push(surface);
        if let Some(capture) = &mut self.segment_capture {
            capture.hoisted.push((capture.read)());
        }
    }

    pub(crate) fn take_backdrops(&mut self) -> Vec<(PaintSurface, Option<Tag>)> {
        let routes = self.segment_capture.as_mut().map(|capture| std::mem::take(&mut capture.hoisted));
        std::mem::take(&mut self.hoisted_backdrops).into_iter().enumerate()
            .map(|(index, surface)| (surface, routes.as_ref().and_then(|routes| routes.get(index)).cloned()))
            .collect()
    }

    pub(crate) fn adopt_backdrops(&mut self, backdrops: Vec<(PaintSurface, Option<Tag>)>) {
        for (surface, routes) in backdrops {
            if self.in_group {
                self.hoisted_backdrops.push(surface);
                if let Some(capture) = &mut self.segment_capture {
                    capture.hoisted.push(routes.unwrap_or_else(|| (capture.read)()));
                }
            } else {
                let before = self.paint_operations.len();
                self.insert_primitive(surface);
                if self.paint_operations.len() > before {
                    if let (Some(capture), Some(routes)) = (&mut self.segment_capture, routes) {
                        *capture.primitives.last_mut().expect("inserted primitive") = routes;
                    }
                }
            }
        }
    }

    pub(super) fn segment_backdrops(&self, output: &mut Scene, id: PaintSegmentId, scale: f32, extent: Bounds<ScaledPixels>, excluded: Option<&Rc<Clip>>) {
        let Some(capture) = &self.segment_capture else { return; };
        for (surface, routes) in self.hoisted_backdrops.iter().zip(&capture.hoisted) {
            if let Some(route) = routes.routes.iter().find(|route| route.id == id) {
                let mut primitive = Primitive::Surface(surface.clone());
                record::geometry::primitive(&mut primitive, (route.destination - route.source).scale(scale), Some(routes.admission(id, scale, Tag::clip_chain(routes.clips.as_ref(), excluded, id, scale, extent))));
                if let Primitive::Surface(surface) = primitive {
                    output.hoisted_backdrops.push(surface);
                }
            }
        }
    }
}
