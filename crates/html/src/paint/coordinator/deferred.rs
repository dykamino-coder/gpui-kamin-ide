//! CSS wrappers переносят только paint, сохраняя layout/prepaint ребёнка.

use super::*;
use crate::hooks::{Hooked, Hooks, IntoHooked};
use gpui::{AnyElement, App, Bounds, IntoElement, LayoutId, Pixels};
use queue::{Inline, Positioned};

pub(super) struct PositionedTag;
static POSITIONED: PositionedTag = PositionedTag;

pub(crate) struct PaintLast {
    child: Option<AnyElement>,
    key: u64,
}
pub(crate) struct PaintInline {
    child: Option<AnyElement>,
}

impl PaintLast {
    pub(crate) fn new(child: AnyElement) -> Self {
        Self {
            child: Some(child),
            key: 0,
        }
    }
    pub(crate) fn key(mut self, key: u64) -> Self {
        self.key = key;
        self
    }
}
impl PaintInline {
    pub(crate) fn new(child: AnyElement) -> Self {
        Self { child: Some(child) }
    }
}

macro_rules! deferred {
    ($ty:ident, $tag:expr, $enqueue:expr) => {
        impl Hooks for $ty {
            type Layout = ();
            type Prepaint = ();
            fn paint_boundary(&self) -> bool {
                false
            }
            fn paint_metadata(&self) -> Option<&dyn std::any::Any> {
                $tag
            }
            fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
                (
                    self.child
                        .as_mut()
                        .expect("unpainted CSS child")
                        .request_layout(window, cx),
                    (),
                )
            }
            fn prepaint(
                &mut self,
                _: Bounds<Pixels>,
                _: &mut (),
                window: &mut Window,
                cx: &mut App,
            ) {
                if let Some(child) = self.child.as_mut() {
                    child.prepaint(window, cx);
                }
            }
            fn paint(
                &mut self,
                _: Bounds<Pixels>,
                _: &mut (),
                _: &mut (),
                window: &mut Window,
                cx: &mut App,
            ) {
                if let Some(pass) = current(window).filter(|pass| pass.is_open(window)) {
                    if let Some(child) = self.child.take() {
                        ($enqueue)(self, child, &pass, window);
                    }
                } else if let Some(child) = self.child.as_mut() {
                    child.paint(window, cx);
                }
            }
        }
        impl IntoElement for $ty {
            type Element = Hooked<Self>;
            fn into_element(self) -> Self::Element {
                self.into_hooked()
            }
        }
    };
}

deferred!(
    PaintLast,
    Some(&POSITIONED),
    |this: &PaintLast, child, pass: &CssPaintPass, window: &Window| {
        let job = Positioned {
            key: this.key,
            child,
            context: window.paint_ctx(),
        };
        pass.jobs.admit(job);
    }
);
deferred!(
    PaintInline,
    None,
    |_: &PaintInline, child, pass: &CssPaintPass, window: &Window| {
        let job = Inline {
            child,
            context: window.raster_paint_ctx(),
        };
        pass.jobs.admit_inline(job);
    }
);
