//! Живое исходное дерево сохраняет LayoutId до единственного прохода краски.

use crate::hooks::{BareRecords, SourceSlices};
use crate::layout::fragment::geometry::Node;
use gpui::{
    AnyElement, App, Bounds, PaintRecordError, PaintSegment, PaintSegmentRecords, Pixels, Window,
    point, px,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub(crate) struct Native {
    root: RefCell<AnyElement>,
    layout: Node,
    extent: Bounds<Pixels>,
    phase: Cell<u8>,
    routing: Cell<Option<(Rc<[PaintSegment]>, SourceSlices)>>,
}

impl Native {
    pub(super) fn new(root: AnyElement, layout: Node, extent: Bounds<Pixels>) -> Self {
        Self {
            root: RefCell::new(root),
            layout,
            extent,
            phase: Cell::new(0),
            routing: Cell::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn prepaint(
        &self,
        segments: Rc<[PaintSegment]>,
        slices: SourceSlices,
        paint: (Rc<[PaintSegment]>, SourceSlices),
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), PaintRecordError> {
        reblessive::Stack::new()
            .enter(|stack| self.prepaint_async(segments, slices, paint, window, cx, stack))
            .finish()
    }

    pub(crate) async fn prepaint_async(
        &self,
        segments: Rc<[PaintSegment]>,
        slices: SourceSlices,
        paint: (Rc<[PaintSegment]>, SourceSlices),
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Result<(), PaintRecordError> {
        self.layout
            .read(window)
            .ok_or(PaintRecordError::InvalidGeometry)?;
        if self.phase.get() != 0 {
            return Err(PaintRecordError::InvalidGeometry);
        }
        self.phase.set(1);
        crate::hooks::prepaint_source_record_async(
            self.extent,
            segments,
            slices,
            window,
            cx,
            stack,
            async |window, cx, stack| {
                self.root
                    .borrow_mut()
                    .prepaint_at_async(point(px(0.), px(0.)), window, cx, stack)
                    .await;
            },
        )
        .await?;
        self.routing.set(Some(paint));
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn record(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(PaintSegmentRecords, BareRecords), PaintRecordError> {
        reblessive::Stack::new()
            .enter(|stack| self.record_async(window, cx, stack))
            .finish()
    }

    pub(crate) async fn record_async(
        &self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Result<(PaintSegmentRecords, BareRecords), PaintRecordError> {
        self.layout
            .read(window)
            .ok_or(PaintRecordError::InvalidGeometry)?;
        if self.phase.get() != 1 {
            return Err(PaintRecordError::InvalidGeometry);
        }
        self.phase.set(2);
        let (segments, slices) = self
            .routing
            .take()
            .ok_or(PaintRecordError::InvalidGeometry)?;
        let extent = slices
            .placed
            .map_or(self.extent, |placed| self.extent.union(&placed));
        crate::hooks::paint_source_record_async(
            extent,
            segments,
            slices,
            window,
            cx,
            stack,
            async |window, cx, stack| {
                self.root.borrow_mut().paint_async(window, cx, stack).await;
            },
        )
        .await
    }
}

#[cfg(test)]
mod tests;
