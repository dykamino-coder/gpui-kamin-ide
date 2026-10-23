//! Областное владение слоями позиционированного paint, пока строятся их потомки.

use super::Positioning;
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
pub(crate) struct LayerScope<'a> {
    positioning: &'a Positioning,
    layer: Layer,
    active: bool,
}

impl<'a> LayerScope<'a> {
    fn new(positioning: &'a Positioning, layer: Layer) -> Self {
        Self {
            positioning,
            layer,
            active: true,
        }
    }

    pub(crate) fn cb(positioning: &'a Positioning, fixed: bool) -> Self {
        positioning.cb_open_with(fixed);
        Self::new(positioning, Layer::Containing)
    }

    pub(crate) fn strict(
        positioning: &'a Positioning,
        style: &crate::style::computed::BoxStyle,
        fixed: bool,
    ) -> Self {
        positioning.cb_open_strict(style, fixed);
        Self::new(positioning, Layer::Containing)
    }

    pub(crate) fn fixed_only(positioning: &'a Positioning) -> Self {
        positioning.cb_open_fixed_only();
        Self::new(positioning, Layer::Containing)
    }

    pub(crate) fn icb(positioning: &'a Positioning) -> Self {
        positioning.icb_open();
        Self::new(positioning, Layer::Initial)
    }

    pub(crate) fn late(positioning: &'a Positioning) -> Self {
        positioning.late_open();
        Self::new(positioning, Layer::Late)
    }

    pub(crate) fn finish(mut self) -> Vec<AnyElement> {
        self.active = false;
        match self.layer {
            Layer::Containing => self.positioning.cb_close(),
            Layer::Initial => self.positioning.icb_close(),
            Layer::Late => self.positioning.late_close(),
        }
    }
}

impl Drop for LayerScope<'_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        match self.layer {
            Layer::Containing => {
                self.positioning.cb.borrow_mut().pop();
            }
            Layer::Initial => {
                self.positioning.icb.borrow_mut().pop();
            }
            Layer::Late => {
                self.positioning.late.borrow_mut().pop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Layer owners finish once and discard unfinished paint while unwinding.

    use super::*;

    fn depths(positioning: &Positioning) -> [usize; 3] {
        [
            positioning.cb.borrow().len(),
            positioning.icb.borrow().len(),
            positioning.late.borrow().len(),
        ]
    }

    #[test]
    fn finish_removes_exactly_its_layer() {
        let positioning = Positioning::default();
        let before = depths(&positioning);
        let outer = LayerScope::cb(&positioning, false);
        let strict = LayerScope::strict(
            &positioning,
            &{
                let mut style = crate::style::computed::BoxStyle::default();
                style.cb_depth = 3;
                style
            },
            true,
        );
        assert_eq!(positioning.cb.borrow().last().unwrap().strict, Some(4));
        assert!(strict.finish().is_empty());
        let fixed = LayerScope::fixed_only(&positioning);
        assert!(fixed.finish().is_empty());
        assert_eq!(depths(&positioning)[0], before[0] + 1);
        assert!(outer.finish().is_empty());
        assert!(LayerScope::icb(&positioning).finish().is_empty());
        assert!(LayerScope::late(&positioning).finish().is_empty());
        assert_eq!(depths(&positioning), before);
    }

    #[test]
    fn unwind_discards_inner_layers_and_preserves_outer_owners() {
        let positioning = Positioning::default();
        let before = depths(&positioning);
        let outer = LayerScope::icb(&positioning);
        let nested = depths(&positioning);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _cb = LayerScope::cb(&positioning, true);
            let _icb = LayerScope::icb(&positioning);
            let _late = LayerScope::late(&positioning);
            panic!("render failed");
        }));
        assert!(result.is_err());
        assert_eq!(depths(&positioning), nested);
        drop(outer);
        assert_eq!(depths(&positioning), before);
    }
}
