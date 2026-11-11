//! The used-value layer as a shared value: elements whose computed style is
//! one object and whose layout edits are the same share one layer.

use super::SharedStyle;
use crate::style::computed::Computed;
use std::rc::Rc;

impl SharedStyle {
    /// The style `used()` reads, when layout's edits go to a layer over it:
    /// the used layer, else the materialized computed style. `None` for a
    /// style of one stage, whose edits change the cascaded style itself.
    pub fn layer(&self) -> Option<Rc<Computed>> {
        match &self.used {
            Some(used) => Some(used.clone()),
            None => self.computed.any(),
        }
    }

    /// Take `used` as the used layer: the result of the edits another element
    /// made over the same `layer()`. The style must have one (`layer`).
    pub fn share_layer(&mut self, used: Rc<Computed>) {
        self.used = Some(used);
    }
}
