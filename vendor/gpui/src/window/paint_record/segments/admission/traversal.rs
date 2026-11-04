//! Допуск сегментов сохраняется до завершения продолжения поддерева.

use super::*;

impl Window {
    /// Удерживает область записи и её допуск до завершения продолжения.
    pub async fn with_paint_segment_slices_async<R>(
        &mut self,
        slices: Rc<[PaintSegmentSlice]>,
        paint: impl AsyncFnOnce(&mut Self) -> R,
    ) -> Result<R, PaintRecordError> {
        if slices.iter().any(|slice| {
            !validation::scaled_bounds_valid(slice.source_clip, self.scale_factor())
                || !validation::scaled_bounds_valid(
                    Bounds::new(
                        slice.source_clip.origin + slice.placement.destination
                            - slice.placement.source,
                        slice.source_clip.size,
                    ),
                    self.scale_factor(),
                )
        }) {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let routes = slices.iter().map(|slice| slice.placement).collect();
        self.with_paint_segments_async(routes, async |window| {
            let _slices = window.render_contexts.enter(Some(Rc::new(Slices(slices))));
            paint(window).await
        })
        .await
    }

    /// Удерживает область записи и её допуск до завершения продолжения.
    pub async fn with_paint_segment_clips_async<R>(
        &mut self,
        clips: &PaintSegmentClips,
        paint: impl AsyncFnOnce(&mut Self) -> R,
    ) -> Result<R, PaintRecordError> {
        clips.identity.check_pass(self)?;
        if self.render_contexts.get::<Recording>().is_none() {
            return Err(PaintRecordError::InvalidSegment);
        }
        if !self
            .render_contexts
            .get::<SegmentSet>()
            .is_some_and(|segments| Rc::ptr_eq(&segments, &clips.segments))
        {
            return Err(PaintRecordError::InvalidSegment);
        }
        let _clips = self.render_contexts.enter(clips.clips.clone());
        Ok(paint(self).await)
    }

    /// Удерживает область записи и её допуск до завершения продолжения.
    pub async fn with_paint_source_clips_async<R>(
        &mut self,
        clips: &PaintSegmentClips,
        base: &PaintSegmentClips,
        paint: impl AsyncFnOnce(&mut Self) -> R,
    ) -> Result<R, PaintRecordError> {
        clips.identity.check_pass(self)?;
        if !Rc::ptr_eq(&clips.segments, &base.segments) {
            return Err(PaintRecordError::InvalidSegment);
        }
        let mut local = Vec::new();
        let mut scope = clips.clips.as_ref();
        while let Some(clip) = scope {
            if base
                .clips
                .as_ref()
                .is_some_and(|base| Rc::ptr_eq(clip, base))
            {
                break;
            }
            local.push(clip.bounds);
            scope = clip.parent.as_ref();
        }
        if scope.is_none() && base.clips.is_some() {
            return Err(PaintRecordError::InvalidSegment);
        }
        if self.render_contexts.get::<Recording>().is_none() {
            return Err(PaintRecordError::InvalidSegment);
        }
        let segments = self
            .render_contexts
            .get::<SegmentSet>()
            .ok_or(PaintRecordError::InvalidSegment)?;
        let routes = scopes(
            &segments.0,
            self.render_contexts.get::<Selection>().as_deref(),
        );
        let mut chain = self.render_contexts.get::<crate::scene::segments::Clip>();
        for bounds in local.into_iter().rev() {
            chain = Some(Rc::new(crate::scene::segments::Clip {
                bounds,
                routes: routes.clone(),
                parent: chain,
            }));
        }
        let _clips = self.render_contexts.enter(chain);
        Ok(paint(self).await)
    }
}
