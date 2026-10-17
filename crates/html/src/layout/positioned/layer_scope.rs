//! Scoped ownership of positioned-paint layers while their children are built.

use super::containing_block::{self as cb, CB, CB_FIXED, CB_STRICT, ICB, LATE};
use crate::style::computed::Computed;
use gpui::AnyElement;

#[derive(Clone, Copy)]
enum Layer {
    Containing,
    Initial,
    Late,
}

/// A layer is either collected with `finish` or discarded on scope exit.
/// Discarding avoids building paint proxies while unwinding a failed render.
#[must_use]
pub(crate) struct LayerScope {
    layer: Layer,
    active: bool,
}

impl LayerScope {
    fn new(layer: Layer) -> Self {
        Self {
            layer,
            active: true,
        }
    }

    pub(crate) fn cb(fixed: bool) -> Self {
        cb::cb_open_with(fixed);
        Self::new(Layer::Containing)
    }

    pub(crate) fn strict(style: &Computed, fixed: bool) -> Self {
        cb::cb_open_strict(style, fixed);
        Self::new(Layer::Containing)
    }

    pub(crate) fn fixed_only() -> Self {
        cb::cb_open_fixed_only();
        Self::new(Layer::Containing)
    }

    pub(crate) fn icb() -> Self {
        cb::icb_open();
        Self::new(Layer::Initial)
    }

    pub(crate) fn late() -> Self {
        cb::late_open();
        Self::new(Layer::Late)
    }

    pub(crate) fn finish(mut self) -> Vec<AnyElement> {
        self.active = false;
        match self.layer {
            Layer::Containing => cb::cb_close(),
            Layer::Initial => cb::icb_close(),
            Layer::Late => cb::late_close(),
        }
    }
}

impl Drop for LayerScope {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        match self.layer {
            Layer::Containing => {
                CB_FIXED.with(|s| {
                    s.borrow_mut().pop();
                });
                CB_STRICT.with(|s| {
                    s.borrow_mut().pop();
                });
                CB.with(|s| {
                    s.borrow_mut().pop();
                });
            }
            Layer::Initial => {
                ICB.with(|s| {
                    s.borrow_mut().pop();
                });
            }
            Layer::Late => {
                LATE.with(|s| {
                    s.borrow_mut().pop();
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn depths() -> [usize; 5] {
        [
            CB.with(|s| s.borrow().len()),
            CB_FIXED.with(|s| s.borrow().len()),
            CB_STRICT.with(|s| s.borrow().len()),
            ICB.with(|s| s.borrow().len()),
            LATE.with(|s| s.borrow().len()),
        ]
    }

    #[test]
    fn finish_removes_exactly_its_layer() {
        let before = depths();
        let outer = LayerScope::cb(false);
        let strict = LayerScope::strict(
            &Computed {
                cb_depth: 3,
                ..Computed::default()
            },
            true,
        );
        assert_eq!(CB_STRICT.with(|s| *s.borrow().last().unwrap()), Some(4));
        assert!(strict.finish().is_empty());
        let fixed = LayerScope::fixed_only();
        assert!(fixed.finish().is_empty());
        assert_eq!(depths()[0], before[0] + 1);
        assert!(outer.finish().is_empty());
        assert!(LayerScope::icb().finish().is_empty());
        assert!(LayerScope::late().finish().is_empty());
        assert_eq!(depths(), before);
    }

    #[test]
    fn unwind_discards_inner_layers_and_preserves_outer_owners() {
        let before = depths();
        let outer = LayerScope::icb();
        let nested = depths();
        let result = std::panic::catch_unwind(|| {
            let _cb = LayerScope::cb(true);
            let _icb = LayerScope::icb();
            let _late = LayerScope::late();
            panic!("render failed");
        });
        assert!(result.is_err());
        assert_eq!(depths(), nested);
        drop(outer);
        assert_eq!(depths(), before);
    }
}
