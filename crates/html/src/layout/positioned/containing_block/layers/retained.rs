//! Назначения исходного стека CB для построения поддерева на prepaint.

use super::super::SpotCell;
use super::super::oof::{Destination, OofKid};
use super::{ABS, CbLayer, FIX, Positioning};
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) struct Retained {
    cb: Vec<(u8, Option<u16>, Destination)>,
    icb: Vec<Destination>,
    late: Vec<Destination>,
    flex_content: Rc<RefCell<std::collections::HashMap<u64, gpui::Bounds<gpui::Pixels>>>>,
}

impl Positioning {
    pub(crate) fn retain(&self) -> Retained {
        Retained {
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
    /// Закрытый стек не открывается заново глобально: у поддерева свой стек,
    /// а назначения старых предков разделяются с их живыми хостами.
    pub(crate) fn resume(&self) -> Rc<Positioning> {
        Rc::new(Positioning {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_destinations_keep_strict_and_fixed_routing_in_host_arena() {
        let p = Positioning::default();
        p.icb_open();
        p.cb_open_with(true);
        p.open_cb(ABS, Some(7));
        let retained = p.retain();
        let strict_host = p.cb_close();
        let fixed_host = p.cb_close();
        let icb_host = p.icb_close();
        let resumed = retained.resume();
        assert!(resumed.can_defer(false, 7, false));
        assert!(resumed.can_defer(true, 4, false));
        assert!(resumed.can_defer(false, 4, true));
        assert_eq!(resumed.cb.borrow()[1].strict, Some(7));
        assert_eq!(resumed.cb.borrow()[0].takes, ABS | FIX);
        use gpui::IntoElement;
        assert!(
            resumed
                .cb_push(Default::default(), gpui::div().into_any_element(), 7)
                .is_none()
        );
        assert!(
            resumed
                .cb_push_fixed(Default::default(), gpui::div().into_any_element())
                .is_none()
        );
        assert!(!resumed.cb.borrow()[1].taken.is_empty());
        assert!(!resumed.cb.borrow()[0].taken.is_empty());
        assert!(
            resumed
                .icb_push(Default::default(), gpui::div().into_any_element())
                .is_none()
        );
        assert!(!resumed.icb.borrow()[0].is_empty());
        drop(resumed);
        drop((strict_host, fixed_host, icb_host));
    }

    #[test]
    fn retained_destinations_do_not_own_closed_queues() {
        let p = Positioning::default();
        p.icb_open();
        p.cb_open_with(true);
        let retained = p.retain();
        p.cb.borrow_mut().clear();
        p.icb.borrow_mut().clear();
        let expired = retained.resume();
        assert!(!expired.cb_active(false));
        assert!(!expired.icb_active());
    }
}
