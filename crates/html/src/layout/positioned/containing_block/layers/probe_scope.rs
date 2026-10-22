//! Измерительная раскладка не публикует OOF во внешние назначения построения.

use super::{OofQueue, Pending, Positioning};

impl Positioning {
    pub(crate) fn isolate(&self) -> ProbeScope<'_> {
        let pending: Vec<_> = self
            .pending
            .borrow()
            .iter()
            .map(|_| Pending::default())
            .collect();
        let local = || OofQueue::with_pending(pending.last().cloned().unwrap_or_default());
        let cb = self
            .cb
            .borrow_mut()
            .iter_mut()
            .map(|l| std::mem::replace(&mut l.taken, local()))
            .collect();
        let icb = self
            .icb
            .borrow_mut()
            .iter_mut()
            .map(|l| std::mem::replace(l, local()))
            .collect();
        let late = self
            .late
            .borrow_mut()
            .iter_mut()
            .map(|l| std::mem::replace(l, local()))
            .collect();
        ProbeScope {
            positioning: self,
            cb,
            icb,
            late,
            pending: std::mem::replace(&mut *self.pending.borrow_mut(), pending),
        }
    }
}

pub(crate) struct ProbeScope<'a> {
    positioning: &'a Positioning,
    cb: Vec<OofQueue>,
    icb: Vec<OofQueue>,
    late: Vec<OofQueue>,
    pending: Vec<Pending>,
}

impl Drop for ProbeScope<'_> {
    fn drop(&mut self) {
        for (layer, saved) in self
            .positioning
            .cb
            .borrow_mut()
            .iter_mut()
            .zip(self.cb.drain(..))
        {
            layer.taken = saved;
        }
        for (layer, saved) in self
            .positioning
            .icb
            .borrow_mut()
            .iter_mut()
            .zip(self.icb.drain(..))
        {
            *layer = saved;
        }
        for (layer, saved) in self
            .positioning
            .late
            .borrow_mut()
            .iter_mut()
            .zip(self.late.drain(..))
        {
            *layer = saved;
        }
        *self.positioning.pending.borrow_mut() = std::mem::take(&mut self.pending);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::IntoElement;

    #[test]
    fn probe_descendants_cannot_reach_retained_live_destinations() {
        let positioning = Positioning::default();
        positioning.icb_open();
        positioning.cb_open_with(true);
        positioning.late_open();
        let live = positioning.retain();
        {
            let _scope = positioning.isolate();
            let probe = positioning.retain().resume();
            assert!(
                probe
                    .cb_push_fixed(Default::default(), gpui::div().into_any_element())
                    .is_none()
            );
            assert!(
                probe
                    .icb_push(Default::default(), gpui::div().into_any_element())
                    .is_none()
            );
            assert!(
                probe
                    .late_push(Default::default(), gpui::div().into_any_element())
                    .is_none()
            );
            assert!(!probe.cb.borrow()[0].taken.is_empty());
            assert!(live.resume().cb.borrow()[0].taken.is_empty());
        }
        assert!(positioning.cb.borrow()[0].taken.is_empty());
        assert!(positioning.icb.borrow()[0].is_empty());
        assert!(positioning.late.borrow()[0].is_empty());
    }
}
