//! CSS wrappers переносят только paint, сохраняя layout/prepaint ребёнка.

use super::*;
use crate::hooks::{Hooked, Hooks, IntoHooked};
use gpui::{AnyElement, App, Bounds, IntoElement, LayoutId, Pixels};
use queue::{Inline, Positioned};

pub(super) struct PositionedTag;
static POSITIONED: PositionedTag = PositionedTag;

/// A positioned root replayed from its native record (`paint::continuation`):
/// its paint needs only the captured window state, so it joins the positioned
/// phase of its stacking context even behind an opaque consumer.
pub(crate) struct ReplayTag;
pub(crate) static REPLAY: ReplayTag = ReplayTag;

pub(super) fn positioned(tag: &dyn std::any::Any) -> bool {
    tag.is::<PositionedTag>() || tag.is::<ReplayTag>()
}

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
    ($ty:ident, $tag:expr, $enqueue:expr, $behind:expr) => {
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
                } else if let Some(child) = self.child.take() {
                    if let Some(mut child) = ($behind)(self, child, window) {
                        child.paint(window, cx);
                        self.child = Some(child);
                    }
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
    },
    |this: &PaintLast, child: AnyElement, window: &Window| {
        // Behind an opaque consumer only a replayed root may still join the
        // collector of its stacking context, in the same composition scope.
        let replayed = child
            .paint_metadata()
            .is_some_and(|tag| tag.is::<ReplayTag>());
        let Some(pass) = current(window).filter(|_| replayed) else {
            return Some(child);
        };
        let job = Positioned {
            key: this.key,
            child,
            context: window.paint_ctx(),
        };
        pass.jobs
            .admit_in(&window.paint_scope_id(), job)
            .map(|job| job.child)
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
    },
    |_: &PaintInline, child: AnyElement, _: &Window| Some(child)
);
