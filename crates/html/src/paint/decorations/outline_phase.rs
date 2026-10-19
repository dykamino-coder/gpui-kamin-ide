//! CSS2 Appendix E: postpone outlines until the current paint phase finishes.

use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, ContentMask, IntoElement, Pixels, Window};
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
        child,
        mode: Mode::Outline,
    }
    .hooked()
}

pub(crate) fn scope(child: AnyElement) -> AnyElement {
    Phase {
        child,
        mode: Mode::Scope,
    }
    .hooked()
}

/// Native opacity is applied by Div while painting its descendants. Keep
/// their outlines in-band rather than replaying outside that opacity scope.
pub(crate) fn in_band(child: AnyElement) -> AnyElement {
    Phase {
        child,
        mode: Mode::InBand,
    }
    .hooked()
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
    child: AnyElement,
    mode: Mode,
}
impl Wrap for Phase {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        match self.mode {
            Mode::Outline => {
                // Inside an outline scope the laid-out child moves to the deferred outline
                // pass (an unlaid `Empty` takes its place and is never painted).
                let deferred = PHASES.with(|phases| match phases.borrow_mut().last_mut() {
                    Some(Some(outlines)) => {
                        let child =
                            std::mem::replace(&mut self.child, gpui::Empty.into_any_element());
                        outlines.push((child, window.content_mask()));
                        true
                    }
                    _ => false,
                });
                if !deferred {
                    self.child.paint(window, cx);
                }
            }
            Mode::Scope => scoped(window, cx, |window, cx| self.child.paint(window, cx)),
            Mode::InBand => {
                PHASES.with(|phases| phases.borrow_mut().push(None));
                self.child.paint(window, cx);
                let _ = PHASES.with(|phases| phases.borrow_mut().pop());
            }
        }
    }
}
