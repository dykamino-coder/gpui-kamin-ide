//! Назначения исходного стека CB для построения поддерева на prepaint.

use super::super::SpotCell;
use super::super::oof::{Destination, OofKid};
use super::{ABS, CbLayer, FIX, Positioning};
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) struct Retained {
    cb: Vec<(u8, Option<u16>, Destination)>,
    pending: Vec<super::Pending>,
    icb: Vec<Destination>,
    late: Vec<Destination>,
    flex_content: Rc<RefCell<std::collections::HashMap<u64, gpui::Bounds<gpui::Pixels>>>>,
}

impl Positioning {
    pub(crate) fn retain(&self) -> Retained {
        Retained {
            pending: self.pending.borrow().clone(),
            cb: self
                .cb
                .borrow()
                .iter()
                .map(|l| (l.takes, l.strict, l.taken.retain()))
                .collect(),
            icb: self.icb.borrow().iter().map(|l| l.retain()).collect(),
            late: self.late.borrow().iter().map(|l| l.retain()).collect(),
            flex_content: self.flex_content.clone(),
        }
    }

    pub(crate) fn can_defer(&self, fixed: bool, nearest: u16, icb: bool) -> bool {
        if icb {
            return self.icb_active();
        }
        self.cb.borrow().iter().any(|l| {
            if fixed {
                l.takes & FIX != 0
            } else {
                l.takes & ABS != 0 && l.strict.is_none_or(|d| d == nearest)
            }
        })
    }

    pub(crate) fn defer(
        &self,
        spot: SpotCell,
        kid: OofKid,
        fixed: bool,
        nearest: u16,
        icb: bool,
    ) -> Result<(), OofKid> {
        let destination = if icb {
            self.icb.borrow().last().cloned()
        } else {
            self.cb
                .borrow()
                .iter()
                .rev()
                .find(|l| {
                    if fixed {
                        l.takes & FIX != 0
                    } else {
                        l.takes & ABS != 0 && l.strict.is_none_or(|d| d == nearest)
                    }
                })
                .map(|l| l.taken.clone())
        };
        match destination {
            Some(queue) => {
                queue.push_kid(spot, kid);
                Ok(())
            }
            None => Err(kid),
        }
    }
}

impl Retained {
    /// Проба получает самостоятельные очереди для каждого внешнего слоя.
    /// Только выбранная проба переподключает их к живым назначениям.
    pub(crate) fn isolate(&self) -> (Rc<Positioning>, Escaped) {
        let mut escaped = Vec::new();
        let mut local = |dest: &Destination| {
            let queue = super::OofQueue::with_pending(dest.upgrade()?.pending());
            escaped.push((queue.clone(), dest.clone()));
            Some(queue)
        };
        let cb = self
            .cb
            .iter()
            .filter_map(|(takes, strict, dest)| {
                Some(CbLayer {
                    takes: *takes,
                    strict: *strict,
                    taken: local(dest)?,
                })
            })
            .collect();
        let icb = self.icb.iter().filter_map(&mut local).collect();
        let late = self.late.iter().filter_map(&mut local).collect();
        let positioning = Rc::new(Positioning {
            cb: RefCell::new(cb),
            pending: RefCell::new(self.pending.clone()),
            icb: RefCell::new(icb),
            late: RefCell::new(late),
            flex_content: Rc::new(RefCell::new(self.flex_content.borrow().clone())),
        });
        (positioning, Escaped(escaped))
    }

    /// Закрытый стек не открывается заново глобально: у поддерева свой стек,
    /// а назначения старых предков разделяются с их живыми хостами.
    pub(crate) fn resume(&self) -> Rc<Positioning> {
        Rc::new(Positioning {
            pending: RefCell::new(self.pending.clone()),
            cb: RefCell::new(
                self.cb
                    .iter()
                    .filter_map(|(takes, strict, dest)| {
                        Some(CbLayer {
                            takes: *takes,
                            strict: *strict,
                            taken: dest.upgrade()?,
                        })
                    })
                    .collect(),
            ),
            icb: RefCell::new(self.icb.iter().filter_map(Destination::upgrade).collect()),
            late: RefCell::new(self.late.iter().filter_map(Destination::upgrade).collect()),
            flex_content: self.flex_content.clone(),
        })
    }
}

pub(crate) struct Escaped(Vec<(super::OofQueue, Destination)>);

impl Escaped {
    /// Измерительные деревья не публикуют выпавших потомков: только дерево,
    /// дошедшее до prepaint, присоединяет их к назначениям исходных предков.
    pub(crate) fn wrap(self, child: gpui::AnyElement) -> gpui::AnyElement {
        use crate::hooks::IntoHooked;
        Published {
            child,
            escaped: Some(self),
        }
        .hooked()
    }

    pub(crate) fn commit(self) {
        for (queue, destination) in self.0 {
            queue.commit_to(&destination);
        }
    }
}

struct Published {
    child: gpui::AnyElement,
    escaped: Option<Escaped>,
}

impl crate::hooks::Wrap for Published {
    fn child(&mut self) -> &mut gpui::AnyElement {
        &mut self.child
    }

    fn prepaint(
        &mut self,
        _: gpui::Bounds<gpui::Pixels>,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        if let Some(escaped) = self.escaped.take() {
            escaped.commit();
        }
        self.child.prepaint(window, cx);
    }
}

#[cfg(test)]
mod tests;
