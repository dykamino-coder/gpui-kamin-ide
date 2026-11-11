//! The flow keys over a rebuild of the same source DOM (`Store::rebase`).

use super::super::Store;

/// How the `html`/`body` boxes of the two trees compared at a rebuild.
#[derive(Clone, Copy)]
pub(super) enum Roots {
    /// Not compared: the flow has no root boxes (unwrapped document).
    Unknown,
    /// Different in layout values: no flow key of the previous tree holds.
    Differ,
    /// Equal in layout values; `true`: in every value.
    Equal(bool),
}

impl Store {
    /// The flow keys over a rebuild: they held the previous document's
    /// `html`/`body` boxes. Boxes the rebuild found equal in layout go
    /// (`FlowKey::strip_boxes`, the verdict stays in `repainted_roots`), a
    /// key whose boxes differ goes whole, as the next build would drop it.
    /// The key before the last serves only a second build of one frame.
    pub(super) fn carry_keys(&self, roots: Roots) {
        self.prev.take();
        let mut last = self.last.take();
        match roots {
            Roots::Unknown => {}
            Roots::Differ => last = None,
            Roots::Equal(equal) => {
                // A key shared with records keeps its boxes: they hold them.
                if let Some(key) = last.as_mut().and_then(std::rc::Rc::get_mut) {
                    key.strip_boxes();
                }
                self.repainted_roots.set(!equal);
            }
        }
        self.last.replace(last);
    }
}
