//! Разветвление некомпозированных команд по явным сегментам перед заморозкой.

use super::*;
use crate::{PaintSegment, PaintSegmentId};
use std::rc::Rc;

mod backdrops;
mod clips;
mod groups;
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
                    if let Some(route) = routes.routes.iter().find(|route| route.id == id) {
                        let clip = Bounds::new(source.origin + route.destination - route.source, source.size);
                        result = Some(result.map_or(clip, |prior: Bounds<Pixels>| prior.union(&clip)));
                    }
                }
            }
        }
        result.unwrap_or(Bounds::new(source.origin, crate::Size::default()))
    }

    pub(crate) fn freeze_segment(&self, id: PaintSegmentId, scale: f32, source: Bounds<Pixels>) -> Scene {
        // Плоский список групп уже упорядочен по зависимостям: повторный обход
        // исходных элементов и промежуточная композиция не нужны.
        let mut groups = Vec::new();
        let mut mapping = Vec::with_capacity(self.groups.len());
        for group in &self.groups {
            let clips = group.scene.common_segment_clips(id, &mapping);
            let (scene, route) = group.scene.segment_commands(id, scale, &mapping, self.segment_clip(id, source).scale(scale), clips.as_ref());
            let mapped = route.map(|route| {
                let offset = (route.destination - route.source).scale(scale);
                let index = groups.len() as u32 + 1;
                let mut copy = Self::copy_group(group, scene, offset);
                // Независимые потомки могут выйти за старый буфер предка.
                // Буфер расширяется с сохранением clip его эффектов.
                if let Some(capture) = &group.scene.segment_capture {
                    for routes in &capture.primitives {
                        if let Some(route) = routes.routes.iter().find(|route| route.id == id) {
                            copy.bounds = copy.bounds.union(&record::geometry::bounds(
                                group.bounds, (route.destination - route.source).scale(scale),
                            ));
                        }
                    }
                }
                for mark in &copy.scene.surfaces {
                    copy.bounds = copy.bounds.union(&mark.bounds);
                }
                let bounds = copy.bounds;
                groups.push(copy);
                Group { index, route, bounds, clips: clips.clone(), atomic: group.scene.segment_capture.as_ref().is_some_and(|capture| capture.atomic) }
            });
            mapping.push(mapped);
        }
        let (mut scene, _) = self.segment_commands(id, scale, &mapping, self.segment_clip(id, source).scale(scale), None);
        scene.groups = groups;
        scene.finish();
        scene
    }

    fn segment_commands(
        &self,
        id: PaintSegmentId,
        scale: f32,
        groups: &[Option<Group>],
        extent: Bounds<ScaledPixels>,
        excluded: Option<&Rc<Clip>>,
    ) -> (Scene, Option<PaintSegment>) {
        let mut output = Scene { in_group: self.in_group, ..Scene::default() };
        let Some(capture) = &self.segment_capture else { return (output, None); };
        let entry = capture.entry.scopes.iter().find(|route| route.id == id).copied();
        let mut first = None;
        let layer_route = entry.or_else(|| capture.primitives.iter()
            .find_map(|routes| routes.routes.iter().find(|route| route.id == id).copied()));
        let mut tags = capture.primitives.iter();
        let mut primitive_index = 0;
        for operation in &self.paint_operations {
            match operation {
                PaintOperation::Primitive(primitive) => {
                    let tag = tags.next().expect("captured primitive tag");
                    let selected = tag.routes.iter().find(|route| route.id == id).copied();
                    let unclipped = capture.unclipped_groups.contains(&primitive_index);
                    primitive_index += 1;
                    let mut primitive = primitive.clone();
                    let mut output_clips = tag.clips.as_ref();
                    let mut shared = false;
                    let route = if let Primitive::Surface(surface) = &mut primitive {
                        if surface.group > 0 {
                            let Some(Some(group)) = groups.get(surface.group as usize - 1) else { continue; };
                            surface.group = group.index;
                            if !group.atomic { output_clips = group.clips.as_ref(); shared = true; }
                            let route = entry.or(selected).unwrap_or(group.route);
                            // Группа уже размещена: дальнейший перенос геометрии
                            // не должен сместить её повторно.
                            surface.bounds = record::geometry::bounds(group.bounds, (route.source - route.destination).scale(scale));
                            if unclipped { surface.content_mask.bounds = surface.bounds; }
                            route
                        } else {
                            let Some(route) = selected else { continue; };
                            route
                        }
                    } else {
                        let Some(route) = selected else { continue; };
                        route
                    };
                    let delta = (route.destination - route.source).scale(scale);
                    let clip = Tag::clip_chain(output_clips, excluded, id, scale, extent);
                    // Композиция предка включает независимо допущенные листья;
                    // его normal-срез не ограничивает их общий выход.
                    let clip = if shared {
                        clip
                    } else { tag.admission(id, scale, clip) };
                    record::geometry::primitive(&mut primitive, delta, Some(clip));
                    output.insert_primitive(primitive);
                    first.get_or_insert(route);
                }
                PaintOperation::StartLayer(bounds) => {
                    let route = layer_route.or(first);
                    let delta = route.map_or(Point::default(), |route| (route.destination - route.source).scale(scale));
                    output.push_layer(record::geometry::bounds(*bounds, delta).union(&extent));
                }
                _ => output.replay_operation(operation.clone()),
            }
        }
        self.segment_backdrops(&mut output, id, scale, extent, excluded);
        output.finish();
        (output, first.map(|route| entry.unwrap_or(route)))
    }
}
