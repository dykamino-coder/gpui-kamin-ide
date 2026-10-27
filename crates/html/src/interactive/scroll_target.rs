//! Цель прокрутки: ручка прокрутки навешивается на саму CSS-коробку, а не на анонимный
//! вьюпорт, который потерял бы её проценты, intrinsic-размеры, baseline и flex/grid-ограничения.
//! Здесь же неподвижные слои scrollport — рамки и канва вьюпорта (CSS Backgrounds 3 §2.11),
//! которые не едут вместе с содержимым.

mod stationary;
use crate::style::computed::Sides;
use gpui::{IntoElement, ParentElement, ScrollHandle, Styled};
use std::cell::Cell;
use std::rc::Rc;

/// Scroll state claimed once by the box carrying the matching node identity.
pub(crate) struct AppliedTarget {
    pub(crate) handle: ScrollHandle,
    pub(crate) horizontal: bool,
    pub(crate) vertical: bool,
    pub(crate) margin: Sides,
}

pub(crate) struct Target {
    node: u64,
    handle: ScrollHandle,
    horizontal: bool,
    vertical: bool,
    margin: Sides,
    applied: Cell<bool>,
}
pub(crate) fn stationary(
    frame: Option<&super::scroll_extent::Frame>,
    target: Option<&Target>,
    node: u64,
    child: gpui::AnyElement,
) -> gpui::AnyElement {
    let handle = target
        .filter(|t| t.applied.get() && t.node == node)
        .map(|t| t.handle.clone());
    match (
        handle,
        crate::interactive::scroll_extent::decoration_clip(frame, node),
    ) {
        (Some(handle), Some(clip)) => stationary::wrap(child, handle, clip),
        _ => child,
    }
}
// CSS Backgrounds 3 §2.11: the canvas covers the viewport, including gutters.
pub(crate) fn canvas(
    frame: Option<&super::scroll_extent::Frame>,
    target: Option<&Target>,
    child: gpui::AnyElement,
) -> gpui::AnyElement {
    let Some((gutter, clip)) = super::scroll_extent::viewport_decoration(frame) else {
        return child;
    };
    let Some(handle) = target.map(|t| t.handle.clone()) else {
        return child;
    };
    let layer = gpui::div()
        .absolute()
        .top(gpui::px(-gutter[1]))
        .left(gpui::px(-gutter[0]))
        .right(gpui::px(-gutter[2]))
        .bottom(gpui::px(-gutter[3]))
        .child(child)
        .into_any_element();
    stationary::wrap(layer, handle, clip)
}

impl Target {
    pub(crate) fn claim(&self, node: u64, style_node: u64) -> Option<AppliedTarget> {
        if self.applied.get() || self.node != node || self.node != style_node {
            return None;
        }
        self.applied.set(true);
        Some(AppliedTarget {
            handle: self.handle.clone(),
            horizontal: self.horizontal,
            vertical: self.vertical,
            margin: self.margin,
        })
    }

    pub(crate) fn new(
        node: u64,
        handle: &ScrollHandle,
        horizontal: bool,
        vertical: bool,
        margin: Sides,
    ) -> Rc<Self> {
        Rc::new(Self {
            node,
            handle: handle.clone(),
            horizontal,
            vertical,
            margin,
            applied: Cell::new(false),
        })
    }

    pub(crate) fn applied(&self) -> bool {
        self.applied.get()
    }
}
