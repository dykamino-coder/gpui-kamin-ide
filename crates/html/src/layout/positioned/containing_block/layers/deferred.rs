//! Снимок внешних CB для построителей, вызываемых после закрытия слоёв сборки.

use super::{CbLayer, OofHost, Positioning};
use std::rc::Rc;

type Pending = OofHost;

pub(crate) struct DeferredPositioning {
    cb: Vec<(u8, Option<u16>, Pending)>,
    icb: Vec<Pending>,
    flex_content:
        Rc<std::cell::RefCell<std::collections::HashMap<u64, gpui::Bounds<gpui::Pixels>>>>,
}

impl Positioning {
    pub(crate) fn isolate(&self) -> ProbeScope<'_> {
        ProbeScope {
            positioning: self,
            cb: self
                .cb
                .borrow_mut()
                .iter_mut()
                .map(|l| std::mem::take(&mut l.taken))
                .collect(),
            icb: self
                .icb
                .borrow_mut()
                .iter_mut()
                .map(std::mem::take)
                .collect(),
        }
    }

    pub(crate) fn defer(&self) -> DeferredPositioning {
        DeferredPositioning {
            flex_content: self.flex_content.clone(),
            cb: self
                .cb
                .borrow_mut()
                .iter_mut()
                .map(|layer| (layer.takes, layer.strict, layer.taken.defer()))
                .collect(),
            icb: self
                .icb
                .borrow_mut()
                .iter_mut()
                .map(OofHost::defer)
                .collect(),
        }
    }
}

pub(crate) struct ProbeScope<'a> {
    positioning: &'a Positioning,
    cb: Vec<OofHost>,
    icb: Vec<OofHost>,
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
    }
}

impl DeferredPositioning {
    /// Рабочие стеки каждого прохода независимы. Число и маски внешних
    /// содержащих блоков совпадают с местом исходного построителя.
    pub(crate) fn fork(&self, publish: bool) -> Rc<Positioning> {
        let positioning = Rc::new(Positioning {
            flex_content: self.flex_content.clone(),
            ..Default::default()
        });
        let pending = |target: &OofHost| {
            if publish {
                target.clear();
                target.borrowed()
            } else {
                OofHost::discard()
            }
        };
        *positioning.cb.borrow_mut() = self
            .cb
            .iter()
            .map(|(takes, strict, target)| CbLayer {
                takes: *takes,
                strict: *strict,
                taken: pending(target),
            })
            .collect();
        *positioning.icb.borrow_mut() = self.icb.iter().map(pending).collect();
        positioning
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{IntoElement, div};

    #[test]
    fn deferred_cb_keeps_routing_but_only_final_build_publishes() {
        let source = Positioning::default();
        source.cb_open_with(true);
        source.icb_open();
        let deferred = source.defer();
        // Исходные слои уже закрыты к моменту вызова построителя хоста.
        assert_eq!(source.cb_close().len(), 1);
        assert_eq!(source.icb_close().len(), 1);
        let trial = deferred.fork(false);
        assert!(trial.cb_active(true));
        assert!(trial.icb_active());
        assert!(
            trial
                .cb_push_fixed(Default::default(), div().into_any_element())
                .is_none()
        );
        assert!(deferred.cb[0].2.is_empty());
        let final_build = deferred.fork(true);
        assert!(
            final_build
                .cb_push_fixed(Default::default(), div().into_any_element())
                .is_none()
        );
        assert!(!deferred.cb[0].2.is_empty());
        {
            let _probe = final_build.isolate();
            let _ = final_build.cb_push_fixed(Default::default(), div().into_any_element());
        }
        assert_eq!(
            deferred.cb[0]
                .2
                .clone()
                .place(super::super::icb_place)
                .len(),
            1
        );
    }
}
