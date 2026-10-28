//! Разветвление некомпозированных команд по явным сегментам перед заморозкой.

use super::*;
use crate::{PaintSegment, PaintSegmentId};
use std::rc::Rc;

mod backdrops;
mod clips;
mod groups;
mod freeze;
mod commands;
use groups::Group;
pub(crate) use clips::{Clip, Tag};

type Routes = Rc<[PaintSegment]>;
type ReadRoutes = Rc<dyn Fn() -> Tag>;

pub(crate) struct Capture {
    read: ReadRoutes,
    entry: Tag,
    primitives: Vec<Tag>,
    hoisted: Vec<Tag>,
    unclipped_groups: Vec<usize>,
    atomic: bool,
}

impl Capture {
    pub(super) fn primitive(&mut self) { self.primitives.push((self.read)()); }
}

impl Scene {
    pub(crate) fn begin_segments(&mut self, read: ReadRoutes) {
        self.segment_capture = Some(Box::new(Capture {
            entry: read(), read, primitives: Vec::new(), hoisted: Vec::new(), unclipped_groups: Vec::new(), atomic: false,
        }));
    }

    pub(crate) fn inherit_segments(&mut self, outer: &Scene) {
        if let Some(capture) = &outer.segment_capture {
            self.begin_segments(capture.read.clone());
        }
    }

    pub(crate) fn bind_record_segments(&mut self, outer: &Scene) {
        if self.segment_capture.is_some() { return; }
        self.inherit_segments(outer);
        if let Some(capture) = &mut self.segment_capture {
            let routes = (capture.read)();
            capture.primitives = self.paint_operations.iter().filter_map(|op| {
                matches!(op, PaintOperation::Primitive(_)).then(|| routes.clone())
            }).collect();
            capture.hoisted = vec![routes; self.hoisted_backdrops.len()];
        }
        // Внешний clip ограничивает выход immutable-поддерева; записанные
        // входы его групп должны оставаться доступны фильтрам.
        if let Some(capture) = &self.segment_capture {
            let read = capture.read.clone();
            let input = Rc::new(move || { let mut tag = read(); tag.clips = None; tag.slices = Rc::from([]); tag });
            for group in &mut self.groups {
                group.scene.begin_segments(input.clone());
                if let Some(capture) = &mut group.scene.segment_capture {
                    capture.atomic = true;
                    let tag = (capture.read)();
                    capture.primitives = group.scene.paint_operations.iter().filter_map(|op| {
                        matches!(op, PaintOperation::Primitive(_)).then(|| tag.clone())
                    }).collect();
                    capture.hoisted = vec![tag; group.scene.hoisted_backdrops.len()];
                }
            }
        }
    }

    pub(crate) fn insert_group_surface(&mut self, surface: PaintSurface, unbounded: bool) {
        let before = self.paint_operations.len();
        self.insert_primitive(surface);
        if unbounded && self.paint_operations.len() > before {
            if let Some(capture) = &mut self.segment_capture {
                capture.unclipped_groups.push(capture.primitives.len() - 1);
            }
        }
    }

    pub(crate) fn segment_clip(&self, id: PaintSegmentId, source: Bounds<Pixels>) -> Bounds<Pixels> {
        let mut result = None;
        for scene in std::iter::once(self).chain(self.groups.iter().map(|group| &group.scene)) {
            if let Some(capture) = &scene.segment_capture {
                for routes in std::iter::once(&capture.entry).chain(&capture.primitives).chain(&capture.hoisted) {
                    for route in routes.routes.iter().filter(|route| route.id == id) {
                        let clip = Bounds::new(source.origin + route.destination - route.source, source.size);
                        result = Some(result.map_or(clip, |prior: Bounds<Pixels>| prior.union(&clip)));
                    }
                }
            }
        }
        result.unwrap_or(Bounds::new(source.origin, crate::Size::default()))
    }

}
