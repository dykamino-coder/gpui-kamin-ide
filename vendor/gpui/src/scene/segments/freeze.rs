//! Экземпляры исходной группы сохраняют общий эффект и явные повторные размещения.

use super::*;

impl Scene {
    pub(crate) fn freeze_segment(&self, id: PaintSegmentId, scale: f32, source: Bounds<Pixels>) -> Scene {
        let extent = self.segment_clip(id, source).scale(scale);
        let mut groups = Vec::new();
        let mut mapping: Vec<Vec<Group>> = Vec::with_capacity(self.groups.len());
        for group in &self.groups {
            let entries: Vec<_> = group.scene.segment_capture.as_ref().into_iter()
                .flat_map(|capture| capture.entry.scopes.iter())
                .filter(|route| route.id == id).copied().collect();
            let mut copies = Vec::new();
            for entry in &entries {
                // Один scope объединяет независимых потомков. Явно повторённый
                // scope получает отдельный эффект для каждого ordinal.
                let ordinal = (entries.len() > 1).then_some(entry.ordinal);
                let clips = group.scene.common_segment_clips(id, ordinal, &mapping);
                let (scene, route) = group.scene.segment_commands(id, ordinal, scale, &mapping, extent, clips.as_ref());
                let Some(route) = route else { continue; };
                let offset = (route.destination - route.source).scale(scale);
                let index = groups.len() as u32 + 1;
                let mut copy = Self::copy_group(group, scene, offset);
                if let Some(capture) = &group.scene.segment_capture {
                    for tag in &capture.primitives {
                        for route in tag.routes.iter().filter(|route| route.id == id
                            && ordinal.is_none_or(|ordinal| route.ordinal == ordinal)) {
                            copy.bounds = copy.bounds.union(&record::geometry::bounds(
                                group.bounds, (route.destination - route.source).scale(scale),
                            ));
                        }
                    }
                }
                for mark in &copy.scene.surfaces { copy.bounds = copy.bounds.union(&mark.bounds); }
                let bounds = copy.bounds;
                groups.push(copy);
                copies.push(Group { index, route, bounds, clips: clips.clone(),
                    atomic: group.scene.segment_capture.as_ref().is_some_and(|capture| capture.atomic) });
            }
            mapping.push(copies);
        }
        let (mut scene, _) = self.segment_commands(id, None, scale, &mapping, extent, None);
        scene.groups = groups;
        scene.finish();
        scene
    }
}
