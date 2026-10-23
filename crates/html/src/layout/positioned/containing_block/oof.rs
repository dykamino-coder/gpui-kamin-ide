//! Носители OOF и очередь слоя, живущая до подготовки кадра.

use super::{DeferredPaint, SpotCell};
use crate::hooks::IntoHooked;
use gpui::{AnyElement, App, Bounds, Pixels, Window};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

pub(crate) type BuildOof = Box<dyn FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> AnyElement>;
pub(crate) type BuildTrials =
    Box<dyn FnOnce(Bounds<Pixels>, SpotCell, Place, &mut Window, &mut App) -> AnyElement>;

/// Готовая геометрия остаётся в исходной раскладке; отложенная строится один раз.
pub(crate) enum OofKid {
    Ready(AnyElement),
    Waiting(crate::layout::positioned::anchor::Dependencies, Box<OofKid>),
    Deferred(BuildOof, Option<DeferredPaint>),
    Trials(BuildTrials, Option<DeferredPaint>),
}

type Item = (SpotCell, OofKid);
pub(crate) type Place = fn(SpotCell, AnyElement) -> AnyElement;

#[derive(Clone)]
pub(super) struct OofQueue {
    items: Rc<RefCell<VecDeque<Item>>>,
    destination: Rc<RefCell<Weak<RefCell<VecDeque<Item>>>>>,
    pending: pending::Pending,
    initial: bool,
}

#[derive(Clone)]
pub(super) struct Destination(Rc<RefCell<Weak<RefCell<VecDeque<Item>>>>>, pending::Pending);

impl Default for OofQueue {
    fn default() -> Self {
        let items = Rc::default();
        Self {
            destination: Rc::new(RefCell::new(Rc::downgrade(&items))),
            items,
            pending: Default::default(),
            initial: false,
        }
    }
}

impl Destination {
    pub(super) fn upgrade(&self) -> Option<OofQueue> {
        self.0.borrow().upgrade().map(|items| OofQueue {
            items,
            destination: self.0.clone(),
            pending: self.1.clone(),
            initial: false,
        })
    }
}

mod host;
mod pending;
pub(super) use pending::Pending;

impl OofQueue {
    pub(super) fn local(
        self,
        spot: SpotCell,
        kid: OofKid,
        cb: u64,
        values: crate::document::state::ValueDoc,
    ) -> AnyElement {
        self.push_kid(spot, kid);
        host::OofHost {
            queue: self,
            place: super::layers::icb_place,
            containing: Some((cb, values)),
        }
        .hooked()
    }

    pub(super) fn with_pending(pending: pending::Pending) -> Self {
        Self {
            pending,
            ..Default::default()
        }
    }

    pub(super) fn pending(&self) -> pending::Pending {
        self.pending.clone()
    }

    pub(super) fn initial() -> Self {
        Self {
            initial: true,
            ..Default::default()
        }
    }

    pub(super) fn retain(&self) -> Destination {
        Destination(self.destination.clone(), self.pending.clone())
    }

    /// Выбранный кандидат отдаёт накопленное назначению предка. Замыкания
    /// его ещё не построенных потомков сохраняют тот же перенаправляемый адрес.
    pub(super) fn commit_to(&self, destination: &Destination) {
        if let Some(target) = destination.upgrade() {
            target.items.borrow_mut().extend(self.items.take());
        }
        *self.destination.borrow_mut() = destination.0.borrow().clone();
    }

    pub(super) fn push(&self, spot: SpotCell, element: AnyElement) {
        self.push_kid(spot, OofKid::Ready(element));
    }

    pub(super) fn push_kid(&self, spot: SpotCell, kid: OofKid) {
        if let OofKid::Waiting(deps, _) = &kid {
            self.pending.observe(deps);
        }
        if let Some(items) = self.destination.borrow().upgrade() {
            items.borrow_mut().push_back((spot, kid));
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.items.borrow().is_empty()
    }

    pub(super) fn place(self, place: Place) -> Vec<AnyElement> {
        let mut ready = Vec::new();
        let mut deferred = VecDeque::new();
        for (spot, kid) in self.items.take() {
            match kid {
                OofKid::Ready(element) => ready.push(place(spot, element)),
                other => deferred.push_back((spot, other)),
            }
        }
        *self.items.borrow_mut() = deferred;
        // Пустое назначение тоже нужно: выпавший потомок может прийти из
        // вложенного хоста позже. Слабые ссылки не образуют цикл с build.
        if self.initial || !self.is_empty() || Rc::strong_count(&self.destination) > 1 {
            ready.push(
                host::OofHost {
                    queue: self,
                    place,
                    containing: None,
                }
                .hooked(),
            );
        }
        ready
    }
}
