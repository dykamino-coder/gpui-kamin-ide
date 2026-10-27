//! Общий внешний clip группы сохраняется после фильтра только для всех её листьев.

use super::*;

pub(super) struct Group {
    pub(super) index: u32,
    pub(super) route: PaintSegment,
    pub(super) bounds: Bounds<ScaledPixels>,
    pub(super) clips: Option<Rc<Clip>>,
    pub(super) atomic: bool,
}

impl Scene {
    pub(super) fn common_segment_clips(&self, id: PaintSegmentId, ordinal: Option<u32>, groups: &[Vec<Group>]) -> Option<Rc<Clip>> {
        let capture = self.segment_capture.as_ref()?;
        let mut common = capture.entry.clips.clone();
        let mut tags = capture.primitives.iter();
        for op in &self.paint_operations {
            let PaintOperation::Primitive(primitive) = op else { continue; };
            let tag = tags.next().expect("captured primitive tag");
            let chain = if let Primitive::Surface(surface) = primitive {
                if surface.group > 0 {
                    let Some(copies) = groups.get(surface.group as usize - 1) else { continue; };
                    for group in copies.iter().filter(|group| ordinal.is_none_or(|ordinal| group.route.ordinal == ordinal)) {
                        retain_common(&mut common, if group.atomic { tag.clips.as_ref() } else { group.clips.as_ref() });
                    }
                    continue;
                } else {
                    if !tag.routes.iter().any(|route| route.id == id && ordinal.is_none_or(|ordinal| route.ordinal == ordinal)) { continue; }
                    tag.clips.as_ref()
                }
            } else {
                if !tag.routes.iter().any(|route| route.id == id && ordinal.is_none_or(|ordinal| route.ordinal == ordinal)) { continue; }
                tag.clips.as_ref()
            };
            retain_common(&mut common, chain);
        }
        for tag in &capture.hoisted {
            if tag.routes.iter().any(|route| route.id == id && ordinal.is_none_or(|ordinal| route.ordinal == ordinal)) {
                retain_common(&mut common, tag.clips.as_ref());
            }
        }
        common
    }
}

fn retain_common(common: &mut Option<Rc<Clip>>, chain: Option<&Rc<Clip>>) {
    while let Some(candidate) = common {
        let mut node = chain;
        while let Some(clip) = node {
            if Rc::ptr_eq(candidate, clip) { return; }
            node = clip.parent.as_ref();
        }
        *common = candidate.parent.clone();
    }
}
