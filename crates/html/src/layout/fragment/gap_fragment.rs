//! Share a column fragment's unused tail with its root grid decoration painter.

use gpui::{Bounds, ContentMask, Pixels};
use std::cell::RefCell;

#[derive(Clone)]
pub(crate) struct Scope {
    pub root: Bounds<Pixels>,
    pub parent: ContentMask<Pixels>,
    pub mask: ContentMask<Pixels>,
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
