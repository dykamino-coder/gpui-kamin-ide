//! Share fragment boundaries with gap painters before resolving decoration insets.

use gpui::{ContentMask, Pixels};
use std::cell::RefCell;

#[derive(Clone)]
pub(crate) struct Scope {
    pub parent: ContentMask<Pixels>,
    pub mask: ContentMask<Pixels>,
    pub start: f32,
    pub cut: f32,
    pub end: f32,
}

thread_local! {
    static ACTIVE: RefCell<Option<Scope>> = const { RefCell::new(None) };
}

pub(crate) fn current() -> Option<Scope> {
    ACTIVE.with(|active| active.borrow().clone())
}

pub(crate) fn with<R>(scope: Option<Scope>, f: impl FnOnce() -> R) -> R {
    let previous = ACTIVE.with(|active| active.replace(scope));
    let result = f();
    ACTIVE.with(|active| active.replace(previous));
    result
}
