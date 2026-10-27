//! Границы сегментов до публикации immutable record; CSS выбирает размещения.

use super::*;
mod admission;
pub use admission::{PaintSegmentClips, PaintSegmentSlice};

/// Идентичность сегмента фрагментации, назначенная хозяином записи.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PaintSegmentId(pub u64);

/// Явное размещение исходных команд в сегменте до композиции групп.
#[derive(Clone, Copy, Debug)]
pub struct PaintSegment {
    /// Идентичность фрагментаинера, выбранная хозяином.
    pub id: PaintSegmentId,
    /// Разрешённая исходная точка в координатах окна.
    pub source: Point<Pixels>,
    /// Целевая точка в системе координат сегмента.
    pub destination: Point<Pixels>,
}

/// Отдельные неизменяемые записи с экземплярами охватывающих эффектов.
pub struct PaintSegmentRecords(Vec<(PaintSegmentId, PaintRecord)>);

impl PaintSegmentRecords {
    /// Чтение сегмента без потребления его повторно используемых команд.
    pub fn get(&self, id: PaintSegmentId) -> Option<&PaintRecord> {
        self.0.iter().find(|(key, _)| *key == id).map(|(_, record)| record)
    }

    /// Упорядоченный набор сегментов, включая пустые записи пустых сегментов.
    pub fn into_records(self) -> Vec<(PaintSegmentId, PaintRecord)> { self.0 }
}

pub(super) struct Recording;

struct Selection(Rc<[PaintSegment]>);
struct SegmentSet(Rc<[PaintSegment]>, Bounds<Pixels>);

fn valid(routes: &[PaintSegment], scale: f32) -> bool {
    routes.iter().enumerate().all(|(index, route)| {
        !routes[..index].iter().any(|other| other.id == route.id)
            && validation::point_valid(route.source)
            && validation::point_valid(route.destination)
            && validation::point_valid(route.destination - route.source)
            && validation::point_valid((route.destination - route.source) * scale)
    })
}

impl Window {
    /// Допуск исходных фрагментов при единственном native paint поддерева.
    /// Отложенные задания сохраняют выбор в paint-контексте. Пустой выбор
    /// исключает собственные команды узла; потомки могут выбрать свои сегменты.
    pub fn with_paint_segments<R>(
        &mut self,
        routes: Rc<[PaintSegment]>,
        paint: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, PaintRecordError> {
        if !valid(&routes, self.scale_factor()) { return Err(PaintRecordError::InvalidGeometry); }
        let Some(segments) = self.render_contexts.get::<SegmentSet>() else {
            return Err(PaintRecordError::InvalidSegment);
        };
        if routes.iter().any(|route| !segments.0.iter().any(|segment| segment.id == route.id)) {
            return Err(PaintRecordError::InvalidSegment);
        }
        if routes.iter().any(|route| !validation::scaled_bounds_valid(
            Bounds::new(segments.1.origin + route.destination - route.source, segments.1.size),
            self.scale_factor(),
        )) { return Err(PaintRecordError::InvalidGeometry); }
        let _selection = self.render_contexts.enter(Some(Rc::new(Selection(routes))));
        let _slices = self.render_contexts.enter::<admission::Slices>(None);
        Ok(paint(self))
    }

    /// Единственный paint маршрутизирует журнал Scene до заморозки PaintRecord.
    /// Охватывающие группы создаются один раз на сегмент и совместно
    /// используются всеми его вложенными листьями.
    pub fn record_paint_segments<R>(
        &mut self,
        source_clip: Bounds<Pixels>,
        routes: Rc<[PaintSegment]>,
        cx: &mut App,
        paint: impl FnOnce(&mut Self, &mut App) -> R,
    ) -> Result<(R, PaintSegmentRecords), PaintRecordError> {
        if !valid(&routes, self.scale_factor()) { return Err(PaintRecordError::InvalidGeometry); }
        if !validation::scaled_bounds_valid(source_clip, self.scale_factor())
            || routes.iter().any(|route| !validation::scaled_bounds_valid(
                Bounds::new(source_clip.origin + route.destination - route.source, source_clip.size),
                self.scale_factor(),
            )) {
            return Err(PaintRecordError::InvalidGeometry);
        }
        let segments = routes.clone();
        let _segments = self.render_contexts.enter(Some(Rc::new(SegmentSet(routes.clone(), source_clip))));
        let _selection = self.render_contexts.enter(Some(Rc::new(Selection(routes))));
        let contexts = self.render_contexts.clone();
        let scope_segments = segments.clone();
        let read = Rc::new(move || crate::scene::segments::Tag {
            routes: contexts.get::<Selection>().map_or_else(|| Rc::from([]), |value| value.0.clone()),
            scopes: scope_segments.iter().map(|root| contexts.get::<Selection>()
                .and_then(|selected| selected.0.iter().find(|route| route.id == root.id).copied())
                .unwrap_or(*root)).collect(),
            clips: contexts.get::<crate::scene::segments::Clip>(),
            slices: contexts.get::<admission::Slices>().map_or_else(|| Rc::from([]), |value| value.0.clone()),
        });
        let (result, draft, identity, scale) = self.capture_paint_commands(source_clip, cx, |window, cx| {
            let _recording = window.render_contexts.enter(Some(Rc::new(Recording)));
            let _clips = window.render_contexts.enter::<crate::scene::segments::Clip>(None);
            let _slices = window.render_contexts.enter::<admission::Slices>(None);
            window.next_frame.scene.begin_segments(read);
            paint(window, cx)
        })?;
        let mut records = Vec::with_capacity(segments.len());
        for segment in segments.iter() {
            let scene = draft.freeze_segment(segment.id, scale, source_clip);
            let clip = draft.segment_clip(segment.id, source_clip);
            if !validation::scaled_bounds_valid(clip, scale) {
                return Err(PaintRecordError::InvalidGeometry);
            }
            records.push((segment.id, PaintRecord(Rc::new(Recorded {
                identity: identity.clone(), scale, clip, scene,
            }))));
        }
        Ok((result, PaintSegmentRecords(records)))
    }
}

impl Window {
    pub(in crate::window) fn segment_capture_mask(&self) -> Option<ContentMask<Pixels>> {
        self.render_contexts.get::<Recording>()?;
        // До размещения нельзя отбрасывать исходные команды по clip предка.
        // Заморозка применяет конечные границы и эффекты каждого сегмента.
        let radius = f32::MAX / 64.0 / self.scale_factor().max(1.0);
        Some(ContentMask { bounds: Bounds::new(point(px(-radius), px(-radius)), size(px(radius * 2.0), px(radius * 2.0))) })
    }

    pub(in crate::window) fn segment_clip_scope(&self, mask: ContentMask<Pixels>, replace: bool) -> Option<render_context::Scope> {
        self.render_contexts.get::<Recording>()?;
        let segments = self.render_contexts.get::<SegmentSet>()?;
        let selected = self.render_contexts.get::<Selection>()?;
        // Scope присутствует во всех сегментах: собственный допуск предка
        // может быть пустым, когда независимый потомок всё ещё рисуется.
        let routes = segments.0.iter().map(|root| selected.0.iter()
            .find(|route| route.id == root.id).copied().unwrap_or(*root)).collect();
        Some(self.render_contexts.enter(Some(Rc::new(crate::scene::segments::Clip {
            bounds: mask.bounds, routes,
            parent: if replace { None } else { self.render_contexts.get() },
        }))))
    }
}
