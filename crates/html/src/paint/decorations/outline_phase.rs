//! CSS2 Appendix E: postpone outlines until the current paint phase finishes.

use gpui::{
    AnyElement, App, Bounds, ContentMask, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window,
};
use std::cell::RefCell;

type Outline = (AnyElement, ContentMask<Pixels>);
thread_local! {
    static PHASES: RefCell<Vec<Option<Vec<Outline>>>> = const { RefCell::new(Vec::new()) };
}

/// Each float and stacking context owns its outlines (Servo paint_traversal).
pub(crate) fn paint(window: &mut Window, cx: &mut App, f: impl FnOnce(&mut Window, &mut App)) {
    PHASES.with(|phases| phases.borrow_mut().push(Some(Vec::new())));
    f(window, cx);
    let outlines = PHASES.with(|phases| phases.borrow_mut().pop().flatten().unwrap());
    for (mut outline, clip) in outlines {
        window.with_content_mask_replaced(clip, |window| outline.paint(window, cx));
    }
}

/// A formatting context does not itself establish a stacking context. Reuse
/// the enclosing phase so nested flow outlines also follow outer floats.
pub(crate) fn flow(window: &mut Window, cx: &mut App, f: impl FnOnce(&mut Window, &mut App)) {
    if PHASES.with(|phases| matches!(phases.borrow().last(), Some(Some(_)))) {
        f(window, cx);
    } else {
        paint(window, cx, f);
    }
}

/// Isolate outlines inside an effect only when an outer flow is collecting.
pub(crate) fn scoped(window: &mut Window, cx: &mut App, f: impl FnOnce(&mut Window, &mut App)) {
    if PHASES.with(|phases| phases.borrow().is_empty()) {
        f(window, cx);
    } else {
        paint(window, cx, f);
    }
}

pub(crate) fn outline(child: AnyElement) -> AnyElement {
    Phase {
        child: Some(child),
        mode: Mode::Outline,
    }
    .into_any_element()
}

pub(crate) fn scope(child: AnyElement) -> AnyElement {
    Phase {
        child: Some(child),
        mode: Mode::Scope,
    }
    .into_any_element()
}

/// Native opacity is applied by Div while painting its descendants. Keep
/// their outlines in-band rather than replaying outside that opacity scope.
pub(crate) fn in_band(child: AnyElement) -> AnyElement {
    Phase {
        child: Some(child),
        mode: Mode::InBand,
    }
    .into_any_element()
}

pub(crate) fn context(child: AnyElement, c: &crate::style::computed::Computed) -> AnyElement {
    if c.opacity.is_some_and(|opacity| opacity < 1.0) {
        in_band(child)
    } else {
        child
    }
}

enum Mode {
    Outline,
    Scope,
    InBand,
}
struct Phase {
    child: Option<AnyElement>,
    mode: Mode,
}
impl IntoElement for Phase {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Phase {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.as_mut().unwrap().request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.as_mut().unwrap().prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        match self.mode {
            Mode::Outline => {
                PHASES.with(|phases| {
                    if let Some(Some(outlines)) = phases.borrow_mut().last_mut() {
                        outlines.push((self.child.take().unwrap(), window.content_mask()));
                    }
                });
                if let Some(child) = self.child.as_mut() {
                    child.paint(window, cx);
                }
            }
            Mode::Scope => scoped(window, cx, |window, cx| {
                self.child.as_mut().unwrap().paint(window, cx)
            }),
            Mode::InBand => {
                PHASES.with(|phases| phases.borrow_mut().push(None));
                self.child.as_mut().unwrap().paint(window, cx);
                let _ = PHASES.with(|phases| phases.borrow_mut().pop());
            }
        }
    }
}
