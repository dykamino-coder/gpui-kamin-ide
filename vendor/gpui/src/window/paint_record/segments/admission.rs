//! Собственный срез узла и цепочка CSS clip отделены от охватывающих эффектов.

use super::*;

/// Допуск и размещение одного узла независимо от срезов его предков.
#[derive(Clone, Copy, Debug)]
pub struct PaintSegmentSlice {
    /// Идентичность сегмента и разрешённое размещение исходной точки.
    pub placement: PaintSegment,
    /// Собственный исходный срез узла до размещения.
    pub source_clip: Bounds<Pixels>,
}

pub(super) struct Slices(pub(super) Rc<[PaintSegmentSlice]>);

/// Сохранённая цепочка clip, выбранная CSS-политикой containing block.
/// Не изменяет opacity, transform, filter или выбор сегментов.
#[derive(Clone)]
pub struct PaintSegmentClips {
    identity: paint_scope::Identity,
    segments: Rc<SegmentSet>,
    clips: Option<Rc<crate::scene::segments::Clip>>,
}

impl Window {
    /// Замена собственного допуска узла. Независимый потомок не наследует
    /// этот срез; охватывающие эффекты остаются общими.
    pub fn with_paint_segment_slices<R>(
        &mut self,
        slices: Rc<[PaintSegmentSlice]>,
        paint: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, PaintRecordError> {
        if slices.iter().any(|slice| !validation::scaled_bounds_valid(slice.source_clip, self.scale_factor())
            || !validation::scaled_bounds_valid(Bounds::new(
                slice.source_clip.origin + slice.placement.destination - slice.placement.source,
                slice.source_clip.size,
            ), self.scale_factor())) {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let routes = slices.iter().map(|slice| slice.placement).collect();
        self.with_paint_segments(routes, |window| {
            let _slices = window.render_contexts.enter(Some(Rc::new(Slices(slices))));
            paint(window)
        })
    }

    /// Сохранение активной цепочки CSS clip на границе containing block.
    pub fn paint_segment_clips(&self) -> Result<PaintSegmentClips, PaintRecordError> {
        if self.render_contexts.get::<Recording>().is_none() {
            return Err(PaintRecordError::InvalidSegment);
        }
        Ok(PaintSegmentClips {
            identity: paint_scope::Identity::capture(self), clips: self.render_contexts.get(),
            segments: self.render_contexts.get::<SegmentSet>().ok_or(PaintRecordError::InvalidSegment)?,
        })
    }

    /// Выбор цепочки clip по containing block без повторной группы эффектов.
    /// Отложенные задания сохраняют выбранную цепочку в своём контексте.
    pub fn with_paint_segment_clips<R>(
        &mut self,
        clips: &PaintSegmentClips,
        paint: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, PaintRecordError> {
        clips.identity.check_pass(self)?;
        if self.render_contexts.get::<Recording>().is_none() {
            return Err(PaintRecordError::InvalidSegment);
        }
        if !self.render_contexts.get::<SegmentSet>().is_some_and(|segments| Rc::ptr_eq(&segments, &clips.segments)) {
            return Err(PaintRecordError::InvalidSegment);
        }
        let _clips = self.render_contexts.enter(clips.clips.clone());
        Ok(paint(self))
    }

    /// Повтор во вложенной записи тех clip, которые цепочка containing block
    /// добавляет поверх `base`. Обе цепочки сохранены одной охватывающей записью
    /// над теми же исходными координатами; каждый clip следует за всеми
    /// выбранными размещениями активной записи, как собственный clip её узлов.
    pub fn with_paint_source_clips<R>(
        &mut self,
        clips: &PaintSegmentClips,
        base: &PaintSegmentClips,
        paint: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, PaintRecordError> {
        clips.identity.check_pass(self)?;
        if !Rc::ptr_eq(&clips.segments, &base.segments) {
            return Err(PaintRecordError::InvalidSegment);
        }
        let mut local = Vec::new();
        let mut scope = clips.clips.as_ref();
        while let Some(clip) = scope {
            if base.clips.as_ref().is_some_and(|base| Rc::ptr_eq(clip, base)) {
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
        let segments = self.render_contexts.get::<SegmentSet>().ok_or(PaintRecordError::InvalidSegment)?;
        let routes = scopes(&segments.0, self.render_contexts.get::<Selection>().as_deref());
        let mut chain = self.render_contexts.get::<crate::scene::segments::Clip>();
        for bounds in local.into_iter().rev() {
            chain = Some(Rc::new(crate::scene::segments::Clip { bounds, routes: routes.clone(), parent: chain }));
        }
        let _clips = self.render_contexts.enter(chain);
        Ok(paint(self))
    }
}
