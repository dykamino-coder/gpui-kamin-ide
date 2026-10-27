//! Передача line-box пробе статической позиции; восстанавливается после prepaint или unwind.

use gpui::Pixels;

/// Each probe receives its paragraph's current line rather than a global slot.
pub type LineBoxSlot = std::rc::Rc<LineBox>;

#[derive(Debug, Default, PartialEq)]
pub struct LineBox {
    paint: std::cell::Cell<Option<(Pixels, Pixels, Pixels)>>,
    pub(crate) normal: std::cell::Cell<Option<super::metrics::static_rect::LineGeometry>>,
}

impl std::ops::Deref for LineBox {
    type Target = std::cell::Cell<Option<(Pixels, Pixels, Pixels)>>;
    fn deref(&self) -> &Self::Target {
        &self.paint
    }
}

/// Expose this line while the probe prepaints, preserving any outer value.
pub fn with_line_box<R>(
    slot: &LineBoxSlot,
    line: (Pixels, Pixels, Pixels),
    f: impl FnOnce() -> R,
) -> R {
    struct Restore<'a>(&'a LineBoxSlot, Option<(Pixels, Pixels, Pixels)>);
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            self.0.set(self.1);
        }
    }
    let _scope = Restore(slot, slot.replace(Some(line)));
    f()
}

#[cfg(test)]
mod tests {
    //! Nested line probes restore their owner after normal return or panic.

    use super::*;
    use gpui::px;

    fn line(top: f32) -> (Pixels, Pixels, Pixels) {
        (px(top), px(12.0), px(3.0))
    }

    #[test]
    fn nested_consumption_restores_the_outer_line() {
        let slot = LineBoxSlot::default();
        assert_eq!(slot.take(), None);
        let answer = with_line_box(&slot, line(1.0), || {
            with_line_box(&slot, line(2.0), || {
                assert_eq!(slot.take(), Some(line(2.0)));
                assert_eq!(slot.take(), None);
            });
            assert_eq!(slot.take(), Some(line(1.0)));
            42
        });
        assert_eq!(answer, 42);
        assert_eq!(slot.take(), None);
    }

    #[test]
    fn unwind_restores_outer_line_and_absent_context() {
        let slot = LineBoxSlot::default();
        assert_eq!(slot.take(), None);
        with_line_box(&slot, line(1.0), || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_line_box(&slot, line(2.0), || {
                    assert_eq!(slot.take(), Some(line(2.0)));
                    panic!("probe failed");
                });
            }));
            assert!(result.is_err());
            assert_eq!(slot.take(), Some(line(1.0)));
        });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_line_box(&slot, line(3.0), || panic!("probe failed"));
        }));
        assert!(result.is_err());
        assert_eq!(slot.take(), None);
    }
}
