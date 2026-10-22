//! Отложенная фаза контуров (CSS 2 Приложение E, шаг 10): `outline` собирается во время
//! покраски и рисуется после завершения текущей фазы — флоата или контекста наложения, как в
//! Servo `paint_traversal`. Поточный контекст форматирования переиспользует внешнюю фазу,
//! эффект изолирует контуры только внутри собирающего потока, а под нативной `opacity` контуры
//! остаются в полосе, чтобы не выйти из её области.

use crate::hooks::{IntoHooked, Wrap};
use crate::paint::context::PaintCx;
use gpui::{AnyElement, App, Bounds, ContentMask, IntoElement, Pixels, Window};
use std::rc::Rc;

pub(crate) type Outline = (AnyElement, ContentMask<Pixels>);
/// Each float and stacking context owns its outlines (Servo paint_traversal).
pub(crate) fn paint(
    owner: Option<&Rc<PaintCx>>,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut Window, &mut App),
) {
    let Some(owner) = owner else {
        return f(window, cx);
    };
    owner.outlines.borrow_mut().push(Some(Vec::new()));
    f(window, cx);
    let outlines = owner
        .outlines
        .borrow_mut()
        .pop()
        .flatten()
        .unwrap_or_default();
    for (mut outline, clip) in outlines {
        window.with_content_mask_replaced(clip, |window| outline.paint(window, cx));
    }
}

/// A formatting context does not itself establish a stacking context. Reuse
/// the enclosing phase so nested flow outlines also follow outer floats.
pub(crate) fn flow(
    owner: Option<&Rc<PaintCx>>,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut Window, &mut App),
) {
    if owner.is_some_and(|owner| matches!(owner.outlines.borrow().last(), Some(Some(_)))) {
        f(window, cx);
    } else {
        paint(owner, window, cx, f);
    }
}

/// Isolate outlines inside an effect only when an outer flow is collecting.
pub(crate) fn scoped(
    owner: Option<&Rc<PaintCx>>,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut Window, &mut App),
) {
    if owner.is_none_or(|owner| owner.outlines.borrow().is_empty()) {
        f(window, cx);
    } else {
        paint(owner, window, cx, f);
    }
}

pub(crate) fn outline(child: AnyElement, owner: Option<&Rc<PaintCx>>) -> AnyElement {
    Phase {
        owner: owner.cloned(),
        child,
        mode: Mode::Outline,
    }
    .hooked()
}

/// The phase of a formatting context around `child` (see [`flow`]).
pub(crate) fn flow_phase(child: AnyElement, owner: Option<&Rc<PaintCx>>) -> AnyElement {
    Phase {
        owner: owner.cloned(),
        child,
        mode: Mode::Flow,
    }
    .hooked()
}

pub(crate) fn scope(child: AnyElement, owner: Option<&Rc<PaintCx>>) -> AnyElement {
    Phase {
        owner: owner.cloned(),
        child,
        mode: Mode::Scope,
    }
    .hooked()
}

/// Native opacity is applied by Div while painting its descendants. Keep
/// their outlines in-band rather than replaying outside that opacity scope.
pub(crate) fn in_band(child: AnyElement, owner: Option<&Rc<PaintCx>>) -> AnyElement {
    Phase {
        owner: owner.cloned(),
        child,
        mode: Mode::InBand,
    }
    .hooked()
}

pub(crate) fn context(
    child: AnyElement,
    c: &crate::style::computed::Computed,
    owner: Option<&Rc<PaintCx>>,
) -> AnyElement {
    if c.opacity.is_some_and(|opacity| opacity < 1.0) {
        in_band(child, owner)
    } else {
        child
    }
}

enum Mode {
    Outline,
    Scope,
    Flow,
    InBand,
}
struct Phase {
    owner: Option<Rc<PaintCx>>,
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
                let deferred = self.owner.as_ref().is_some_and(|owner| {
                    match owner.outlines.borrow_mut().last_mut() {
                        Some(Some(outlines)) => {
                            let child =
                                std::mem::replace(&mut self.child, gpui::Empty.into_any_element());
                            outlines.push((child, window.content_mask()));
                            true
                        }
                        _ => false,
                    }
                });
                if !deferred {
                    self.child.paint(window, cx);
                }
            }
            // Outline isolation is not a paint-order boundary: the scoped
            // float's own atomic paint keeps its inline content and passes
            // positioned descendants to the enclosing stacking context
            // (CSS 2.1 Appendix E.2 step 5).
            Mode::Scope => gpui::paint_reopen(|| {
                scoped(self.owner.as_ref(), window, cx, |window, cx| {
                    self.child.paint(window, cx)
                })
            }),
            Mode::Flow => flow(self.owner.as_ref(), window, cx, |window, cx| {
                self.child.paint(window, cx)
            }),
            Mode::InBand => {
                if let Some(owner) = &self.owner {
                    owner.outlines.borrow_mut().push(None);
                }
                self.child.paint(window, cx);
                if let Some(owner) = &self.owner {
                    owner.outlines.borrow_mut().pop();
                }
            }
        }
    }
}
