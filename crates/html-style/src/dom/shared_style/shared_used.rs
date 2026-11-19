//! A used-value layer shared between boxes: installed whole, never copied.

use super::SharedStyle;
use crate::style::computed::Computed;
use std::rc::Rc;

impl SharedStyle {
    /// Install a used-value layer that other boxes of the same computed style
    /// share (the document's table of used styles), instead of writing through
    /// `used_mut`, which copies the whole `Computed`.
    pub fn set_used_rc(&mut self, used: Rc<Computed>) {
        self.used = Some(used);
    }
}
