//! Границы текущего фрагмента для художников зазоров (`paint::gap_rules`): thread-local
//! `Scope` с маской фрагмента, маской родителя и начальной, срезанной и конечной координатой,
//! который стопка колонок ставит на время краски ребёнка — украшения зазоров режутся по
//! фрагменту до разрешения своих отступов.

use crate::layout::cx::deferred::Deferred;
use gpui::{ContentMask, Pixels};

#[derive(Clone)]
pub(crate) struct Scope {
    pub parent: ContentMask<Pixels>,
    pub mask: ContentMask<Pixels>,
    pub start: f32,
    pub cut: f32,
    pub end: f32,
}

pub(crate) fn current(owner: &Deferred) -> Option<Scope> {
    owner.gap.borrow().clone()
}

pub(crate) fn with<R>(owner: &Deferred, scope: Option<Scope>, f: impl FnOnce() -> R) -> R {
    let previous = owner.gap.replace(scope);
    let result = f();
    owner.gap.replace(previous);
    result
}
