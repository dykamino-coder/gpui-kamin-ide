//! Проекция каждого допуска выполняется до композиции неизменяемого сегмента.

use super::*;

impl Scene {
    pub(super) fn segment_commands(
        &self, id: PaintSegmentId, ordinal: Option<u32>, scale: f32,
        groups: &[Vec<Group>], extent: Bounds<ScaledPixels>, excluded: Option<&Rc<Clip>>,
    ) -> (Scene, Option<PaintSegment>) {
        let mut output = Scene { in_group: self.in_group, ..Scene::default() };
        let Some(capture) = &self.segment_capture else { return (output, None); };
        let admits = |route: &&PaintSegment| route.id == id && ordinal.is_none_or(|ordinal| route.ordinal == ordinal);
        let entry = capture.entry.scopes.iter().find(admits).copied();
        let mut first = None;
        let layer_route = entry.or_else(|| capture.primitives.iter()
            .find_map(|tag| tag.routes.iter().find(admits).copied()));
        let mut tags = capture.primitives.iter();
        let mut primitive_index = 0;
        for operation in &self.paint_operations {
            match operation {
                PaintOperation::Primitive(primitive) => {
                    let tag = tags.next().expect("captured primitive tag");
                    let unclipped = capture.unclipped_groups.contains(&primitive_index);
                    primitive_index += 1;
                    if let Primitive::Surface(surface) = primitive {
                        if surface.group > 0 {
                            let Some(copies) = groups.get(surface.group as usize - 1) else { continue; };
                            for group in copies.iter().filter(|group| ordinal.is_none_or(|ordinal| group.route.ordinal == ordinal)) {
                                let mut primitive = primitive.clone();
                                let Primitive::Surface(surface) = &mut primitive else { unreachable!() };
                                surface.group = group.index;
                                let route = if copies.len() > 1 { group.route } else {
                                    entry.or_else(|| tag.routes.iter().find(admits).copied()).unwrap_or(group.route)
                                };
                                // Вход группы уже размещён; выход не получает второй перенос.
                                surface.bounds = record::geometry::bounds(group.bounds, (route.source - route.destination).scale(scale));
                                if unclipped { surface.content_mask.bounds = surface.bounds; }
                                let clips = if group.atomic { tag.clips.as_ref() } else { group.clips.as_ref() };
                                let clip = Tag::clip_chain(clips, excluded, route, scale, extent);
                                let clip = if group.atomic { tag.admission(route, scale, clip) } else { clip };
                                record::geometry::primitive(&mut primitive, (route.destination - route.source).scale(scale), Some(clip));
                                output.insert_primitive(primitive);
                                first.get_or_insert(route);
                            }
                            continue;
                        }
                    }
                    for route in tag.routes.iter().filter(admits).copied() {
                        let mut primitive = primitive.clone();
                        let clip = Tag::clip_chain(tag.clips.as_ref(), excluded, route, scale, extent);
                        let clip = tag.admission(route, scale, clip);
                        record::geometry::primitive(&mut primitive, (route.destination - route.source).scale(scale), Some(clip));
                        output.insert_primitive(primitive);
                        first.get_or_insert(route);
                    }
                }
                PaintOperation::StartLayer(bounds) => {
                    let route = layer_route.or(first);
                    let delta = route.map_or(Point::default(), |route| (route.destination - route.source).scale(scale));
                    output.push_layer(record::geometry::bounds(*bounds, delta).union(&extent));
                }
                _ => output.replay_operation(operation.clone()),
            }
        }
        self.segment_backdrops(&mut output, id, ordinal, scale, extent, excluded);
        output.finish();
        (output, first.map(|route| entry.unwrap_or(route)))
    }
}
