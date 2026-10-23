//! GPUI-элемент `ScrollArea` для `overflow: auto | scroll`: держит между кадрами ручку
//! прокрутки, кадр прокручиваемого переполнения (`scroll_extent::Frame`) и состояние полос,
//! передаёт дочернему дереву неокруглённое смещение прокрутки (CSSOM View §6), после
//! измерения переполнения просит перерисовку, красит полосы и их области попадания.

use crate::document::state::ValueCx;
use gpui::{AnyElement, ElementId};
mod lifecycle;
use std::rc::Rc;

type ScrollBuilder =
    Rc<dyn Fn(&gpui::ScrollHandle, bool, bool, Option<super::scroll_extent::Frame>) -> AnyElement>;

/// Сохраняет ручку GPUI между кадрами и строит содержимое с прокруткой.
pub struct ScrollArea {
    pub(crate) id: ElementId,
    pub(crate) horizontal: bool,
    pub(crate) vertical: bool,
    pub(crate) style: crate::style::computed::BoxStyle,
    tree: Option<Rc<crate::dom::Element>>,
    parent_extent: Option<super::scroll_extent::Frame>,
    /// Shift chain the area is built in (`ShiftLink`): its border box and
    /// scrollport are recorded without those paint-time shifts.
    sticky: Option<crate::interactive::scroll_extent::StickyShift>,
    pub(crate) build: ScrollBuilder,
    /// Values of the document the area's style lengths come from.
    values: crate::document::state::ValueDoc,
    freeze_scrollbars: bool,
}

impl ScrollArea {
    pub fn new(
        id: ElementId,
        horizontal: bool,
        vertical: bool,
        build: ScrollBuilder,
        vcx: ValueCx<'_>,
    ) -> Self {
        ScrollArea {
            values: vcx.keep(),
            freeze_scrollbars: false,
            id,
            horizontal,
            vertical,
            build,
            style: Default::default(),
            tree: None,
            parent_extent: None,
            sticky: None,
        }
    }
    pub(crate) fn with_style(
        mut self,
        style: crate::style::computed::BoxStyle,
        freeze: bool,
    ) -> Self {
        self.freeze_scrollbars = freeze;
        self.style = style;
        self
    }

    pub(crate) fn with_tree(
        mut self,
        e: &crate::dom::Element,
        frame: Option<&super::scroll_extent::Frame>,
    ) -> Self {
        self.parent_extent = super::scroll_extent::Frame::parent_for(frame, e.node_id);
        self.tree = Some(Rc::new(e.clone()));
        self
    }

    pub(crate) fn with_sticky(
        mut self,
        sticky: Option<crate::interactive::scroll_extent::StickyShift>,
    ) -> Self {
        self.sticky = sticky;
        self
    }
}

#[derive(Clone, Default)]
struct Persistent {
    handle: gpui::ScrollHandle,
    extent: super::scroll_extent::Frame,
    interaction: super::scrollbars::interaction::Interaction,
}
