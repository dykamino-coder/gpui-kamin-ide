//! Допуск сегментов сохраняется до завершения продолжения поддерева.

use super::*;

impl Window {
    /// Удерживает область записи и её допуск до завершения продолжения.
    pub async fn with_paint_segments_async<R>(
        &mut self,
        routes: Rc<[PaintSegment]>,
        paint: impl AsyncFnOnce(&mut Self) -> R,
    ) -> Result<R, PaintRecordError> {
        if !valid(&routes, self.scale_factor()) {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let Some(segments) = self.render_contexts.get::<SegmentSet>() else {
            return Err(PaintRecordError::InvalidSegment);
        };
        if routes
            .iter()
            .any(|route| !segments.0.iter().any(|segment| segment.id == route.id))
        {
            return Err(PaintRecordError::InvalidSegment);
        }
        if routes.iter().any(|route| {
            !validation::scaled_bounds_valid(
                Bounds::new(
                    segments.1.origin + route.destination - route.source,
                    segments.1.size,
                ),
                self.scale_factor(),
            )
        }) {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let _selection = self.render_contexts.enter(Some(Rc::new(Selection(routes))));
        let _slices = self.render_contexts.enter::<admission::Slices>(None);
        Ok(paint(self).await)
    }

    /// Удерживает область записи и её допуск до завершения продолжения.
    pub async fn record_paint_segments_async<R>(
        &mut self,
        source_clip: Bounds<Pixels>,
        routes: Rc<[PaintSegment]>,
        cx: &mut App,
        stack: &mut crate::PaintStack,
        paint: impl AsyncFnOnce(&mut Self, &mut App, &mut crate::PaintStack) -> R,
    ) -> Result<(R, PaintSegmentRecords), PaintRecordError> {
        if !valid(&routes, self.scale_factor()) {
            return Err(PaintRecordError::InvalidGeometry);
        }
        if !validation::scaled_bounds_valid(source_clip, self.scale_factor())
            || routes.iter().any(|route| {
                !validation::scaled_bounds_valid(
                    Bounds::new(
                        source_clip.origin + route.destination - route.source,
                        source_clip.size,
                    ),
                    self.scale_factor(),
                )
            })
        {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let segments = routes.clone();
        let _segments = self
            .render_contexts
            .enter(Some(Rc::new(SegmentSet(routes.clone(), source_clip))));
        let _selection = self.render_contexts.enter(Some(Rc::new(Selection(routes))));
        let contexts = self.render_contexts.clone();
        let scope_segments = segments.clone();
        let read = Rc::new(move || crate::scene::segments::Tag {
            routes: contexts
                .get::<Selection>()
                .map_or_else(|| Rc::from([]), |value| value.0.clone()),
            scopes: scopes(&scope_segments, contexts.get::<Selection>().as_deref()),
            clips: contexts.get::<crate::scene::segments::Clip>(),
            slices: contexts
                .get::<admission::Slices>()
                .map_or_else(|| Rc::from([]), |value| value.0.clone()),
        });
        let (result, draft, identity, scale) = self
            .capture_paint_commands_async(source_clip, cx, stack, async |window, cx, stack| {
                let _recording = window.render_contexts.enter(Some(Rc::new(Recording)));
                let _clips = window
                    .render_contexts
                    .enter::<crate::scene::segments::Clip>(None);
                let _slices = window.render_contexts.enter::<admission::Slices>(None);
                window.next_frame.scene.begin_segments(read);
                paint(window, cx, stack).await
            })
            .await?;
        let mut records = Vec::with_capacity(segments.len());
        for (index, segment) in segments.iter().enumerate() {
            if segments[..index].iter().any(|prior| prior.id == segment.id) {
                continue;
            }
            let scene = draft.freeze_segment(segment.id, scale, source_clip);
            let clip = draft.segment_clip(segment.id, source_clip);
            if !validation::scaled_bounds_valid(clip, scale) {
                return Err(PaintRecordError::InvalidGeometry);
            }
            records.push((
                segment.id,
                PaintRecord(Rc::new(Recorded {
                    identity: identity.clone(),
                    scale,
                    clip,
                    scene,
                })),
            ));
        }
        Ok((result, PaintSegmentRecords(records)))
    }
}
