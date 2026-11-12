//! Последовательные наблюдатели одной коробки без дополнительного layout-узла.

use super::*;

struct Sequence(Box<dyn LayoutObserver>, Box<dyn LayoutObserver>);

impl LayoutObserver for Sequence {
    fn request_layout(&mut self, id: LayoutId, window: &Window) {
        self.0.request_layout(id, window);
        self.1.request_layout(id, window);
    }

    fn prepaint(&mut self, window: &mut Window) -> bool {
        self.0.prepaint(window) && self.1.prepaint(window)
    }
}

impl Div {
    /// Append an observer after the existing layout-input observer.
    pub fn also_observe_layout(mut self, observer: impl LayoutObserver) -> Self {
        let observer: Box<dyn LayoutObserver> = Box::new(observer);
        self.layout_observer = Some(match self.layout_observer.take() {
            Some(previous) => Box::new(Sequence(previous, observer)),
            None => observer,
        });
        self
    }
}
