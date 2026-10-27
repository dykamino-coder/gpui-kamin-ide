//! Живое исходное дерево сохраняет LayoutId до единственного прохода краски.

use crate::hooks::SourceSlices;
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

    pub(crate) fn prepaint(
        &self,
        segments: Rc<[PaintSegment]>,
        slices: SourceSlices,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), PaintRecordError> {
        self.layout
            .read(window)
            .ok_or(PaintRecordError::InvalidGeometry)?;
        if self.phase.get() != 0 {
            return Err(PaintRecordError::InvalidGeometry);
        }
        self.phase.set(1);
        crate::hooks::prepaint_source_record(
            self.extent,
            segments.clone(),
            slices.clone(),
            window,
            cx,
            |window, cx| {
                self.root
                    .borrow_mut()
                    .prepaint_at(point(px(0.), px(0.)), window, cx);
            },
        )?;
        self.routing.set(Some((segments, slices)));
        Ok(())
    }

    pub(crate) fn record(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<PaintSegmentRecords, PaintRecordError> {
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
        crate::hooks::paint_source_record(
            self.extent,
            segments,
            slices,
            window,
            cx,
            |window, cx| {
                self.root.borrow_mut().paint(window, cx);
            },
        )
    }
}

#[cfg(test)]
mod tests;
