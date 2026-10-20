//! Share fragment boundaries with gap painters before resolving decoration insets.

use crate::paint::context::PaintCx;
use gpui::{ContentMask, Pixels};

#[derive(Clone)]
pub(crate) struct Scope {
    pub parent: ContentMask<Pixels>,
    pub mask: ContentMask<Pixels>,
    pub start: f32,
    pub cut: f32,
    pub end: f32,
}

pub(crate) fn current(owner: &PaintCx) -> Option<Scope> {
    owner.gap.borrow().clone()
}

pub(crate) fn with<R>(owner: &PaintCx, scope: Option<Scope>, f: impl FnOnce() -> R) -> R {
    let previous = owner.gap.replace(scope);
    let result = f();
    owner.gap.replace(previous);
    result
}
