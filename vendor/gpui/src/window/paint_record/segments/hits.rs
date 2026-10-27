//! Один исходный prepaint выпускает hitboxes в тех же размещениях, что и paint.

use super::*;

struct HitRecording;

impl Window {
    /// Исходный prepaint сохраняет родные идентичности hitbox и их обработчики.
    /// Размещения выбираются теми же with_paint_segment_slices, что при paint.
    pub fn with_prepaint_segments<R>(
        &mut self, source_clip: Bounds<Pixels>, routes: Rc<[PaintSegment]>, cx: &mut App,
        prepaint: impl FnOnce(&mut Self, &mut App) -> R,
    ) -> Result<R, PaintRecordError> {
        paint_scope::Identity::capture(self).check_pass(self)?;
        if self.invalidator.inner.borrow().draw_phase != DrawPhase::Prepaint {
            return Err(PaintRecordError::InvalidSegment);
        }
        if !valid(&routes, self.scale_factor())
            || !validation::scaled_bounds_valid(source_clip, self.scale_factor())
            || routes.iter().any(|route| !validation::scaled_bounds_valid(
                Bounds::new(source_clip.origin + route.destination - route.source, source_clip.size),
                self.scale_factor(),
            )) { return Err(PaintRecordError::InvalidGeometry); }
        let _recording = self.render_contexts.enter(Some(Rc::new(Recording)));
        let _hits = self.render_contexts.enter(Some(Rc::new(HitRecording)));
        let _segments = self.render_contexts.enter(Some(Rc::new(SegmentSet(routes.clone(), source_clip))));
        let _selection = self.render_contexts.enter(Some(Rc::new(Selection(routes))));
        let _clips = self.render_contexts.enter::<crate::scene::segments::Clip>(None);
        let _slices = self.render_contexts.enter::<admission::Slices>(None);
        let result = prepaint(self, cx);
        paint_scope::Identity::capture(self).check_pass(self)?;
        Ok(result)
    }

    pub(in crate::window) fn insert_segment_hitbox(&mut self, hitbox: &Hitbox) -> bool {
        if self.render_contexts.get::<HitRecording>().is_none() { return false; }
        let Some(segments) = self.render_contexts.get::<SegmentSet>() else { return false; };
        let tag = crate::scene::segments::Tag {
            routes: self.render_contexts.get::<Selection>().map_or_else(|| Rc::from([]), |value| value.0.clone()),
            scopes: Rc::from([]), clips: self.render_contexts.get(),
            slices: self.render_contexts.get::<admission::Slices>().map_or_else(|| Rc::from([]), |value| value.0.clone()),
        };
        for route in tag.routes.iter().copied() {
            let delta = route.destination - route.source;
            let extent = Bounds::new(segments.1.origin + delta, segments.1.size).scale(1.0);
            let clip = crate::scene::segments::Tag::clip_chain(tag.clips.as_ref(), None, route, 1.0, extent);
            let clip = tag.admission(route, 1.0, clip).map(|value| px(value.0));
            let mut placed = hitbox.clone();
            placed.bounds.origin += delta;
            placed.content_mask = ContentMask { bounds: clip };
            if !placed.bounds.intersect(&clip).is_empty() {
                self.note_segment_hit_scope(route);
                self.next_frame.hitboxes.push(placed);
            }
        }
        true
    }

    /// Фактический выбор курсора тем же hit test, который использует платформа.
    #[cfg(any(test, feature = "test-support"))]
    pub fn hit_test_cursor_style(&self) -> CursorStyle {
        self.rendered_frame.cursor_style(self).unwrap_or(CursorStyle::Arrow)
    }
}
