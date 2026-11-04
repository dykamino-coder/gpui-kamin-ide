//! Отложенная фаза контуров (CSS 2 Приложение E, шаг 10): `outline` собирается во время
//! покраски и рисуется после завершения текущей фазы — флоата или контекста наложения, как в
//! Servo `paint_traversal`. Поточный контекст форматирования переиспользует внешнюю фазу,
//! эффект изолирует контуры только внутри собирающего потока, а под нативной `opacity` контуры
//! остаются в полосе, чтобы не выйти из её области.

use crate::hooks::{IntoHooked, Wrap};
use crate::layout::cx::deferred::Deferred;
use gpui::{AnyElement, App, Bounds, IntoElement, Pixels, Window};
use std::rc::Rc;
mod traversal;
pub(crate) use traversal::{flow_async, scoped_async};

pub(crate) fn outline(child: AnyElement, owner: Option<&Rc<Deferred>>) -> AnyElement {
    Phase {
        owner: owner.cloned(),
        child,
        mode: Mode::Outline,
    }
    .hooked()
}

/// The phase of a formatting context around `child` (see [`flow_async`]).
pub(crate) fn flow_phase(child: AnyElement, owner: Option<&Rc<Deferred>>) -> AnyElement {
    Phase {
        owner: owner.cloned(),
        child,
        mode: Mode::Flow,
    }
    .hooked()
}

pub(crate) fn scope(child: AnyElement, owner: Option<&Rc<Deferred>>) -> AnyElement {
    Phase {
        owner: owner.cloned(),
        child,
        mode: Mode::Scope,
    }
    .hooked()
}

/// Native opacity is applied by Div while painting its descendants. Keep
/// their outlines in-band rather than replaying outside that opacity scope.
pub(crate) fn in_band(child: AnyElement, owner: Option<&Rc<Deferred>>) -> AnyElement {
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
    owner: Option<&Rc<Deferred>>,
) -> AnyElement {
    if c.reset_effects.opacity.is_some_and(|opacity| opacity < 1.0) {
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
    owner: Option<Rc<Deferred>>,
    child: AnyElement,
    mode: Mode,
}

impl Phase {
    // Замена элемента относится только к отложенному outline; её временные
    // значения не должны увеличивать рекурсивные кадры Flow и Scope.
    #[inline(never)]
    async fn paint_outline(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        let deferred =
            self.owner
                .as_ref()
                .is_some_and(|owner| match owner.outlines.borrow_mut().last_mut() {
                    Some(Some(outlines)) => {
                        let child =
                            std::mem::replace(&mut self.child, gpui::Empty.into_any_element());
                        outlines.push((child, window.content_mask()));
                        true
                    }
                    _ => false,
                });
        if !deferred {
            self.child.paint_async(window, cx, stack).await;
        }
    }
}

impl Wrap for Phase {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    async fn paint_async(
        &mut self,
        _: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        match self.mode {
            Mode::Outline => self.paint_outline(window, cx, stack).await,
            // Outline isolation is not a paint-order boundary: the scoped
            // float's own atomic paint keeps its inline content and passes
            // positioned descendants to the enclosing stacking context
            // (CSS 2.1 Appendix E.2 step 5).
            Mode::Scope => {
                crate::hooks::paint_reopen_async(window, async |window| {
                    scoped_async(
                        self.owner.as_ref(),
                        window,
                        cx,
                        stack,
                        async |window, cx, stack| self.child.paint_async(window, cx, stack).await,
                    )
                    .await
                })
                .await
            }
            Mode::Flow => {
                flow_async(
                    self.owner.as_ref(),
                    window,
                    cx,
                    stack,
                    async |window, cx, stack| self.child.paint_async(window, cx, stack).await,
                )
                .await
            }
            Mode::InBand => {
                if let Some(owner) = &self.owner {
                    owner.outlines.borrow_mut().push(None);
                }
                self.child.paint_async(window, cx, stack).await;
                if let Some(owner) = &self.owner {
                    owner.outlines.borrow_mut().pop();
                }
            }
        }
    }
}
